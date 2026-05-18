# Ferryman Architecture

> 轻量级 CLI 大数据格式转换工具 — 设计架构文档

## 1. 设计理念

**单一中枢模型**：Apache Arrow `RecordBatch` 是所有格式互转的中间表示。每个格式仅需实现两个 trait——`FormatReader`（文件 → Arrow 流）和 `FormatWriter`（Arrow 流 → 文件），即可与其他所有格式互转。

```
         ┌──────┐   ┌──────┐   ┌──────┐   ┌──────┐
         │ CSV  │   │ JSON │   │Excel │   │ORC   │  ...
         └──┬───┘   └──┬───┘   └──┬───┘   └──┬───┘
            │          │          │          │
    FormatReader   FormatReader   ...        ...
            │          │          │          │
            └──────────┼──────────┼──────────┘
                       ▼
              ┌─────────────────┐
              │ Arrow RecordBatch│  ◄── 唯一中枢
              └────────┬────────┘
                       │
            ┌──────────┼──────────┐
            │          │          │
    FormatWriter   FormatWriter   ...
            │          │          │
         ┌──┴───┐   ┌──┴───┐   ┌──┴───┐
         │ CSV  │   │ JSON │   │Excel │  ...
         └──────┘   └──────┘   └──────┘
```

- **O(N+M)** 复杂度：N 个 Reader + M 个 Writer，不需要 N×M 个转换器
- 新增格式只需实现两个 trait，自动与所有已有格式互通

---

## 2. 系统分层

```
┌─────────────────────────────────────────────────────────┐
│                     CLI Layer (clap)                     │
│  fm convert --from csv --to parquet [OPTIONS] in out     │
│  fm list-formats   |   fm detect <FILE>                  │
├─────────────────────────────────────────────────────────┤
│                   Dispatch & Config                      │
│  register_all_formats() → build_reader() + build_writer() │
│  ConvertConfig → ConversionEngine                        │
├─────────────────────────────────────────────────────────┤
│                   Conversion Engine                      │
│  ┌──────────┐   ┌───────────┐   ┌──────────┐           │
│  │ ModeDetect│   │SchemaAdapt│   │Partition │           │
│  │stream/mem│   │ typed→str  │   │ rows/size│           │
│  └──────────┘   └───────────┘   └──────────┘           │
│                                                         │
│  Reader ────────► Arrow RecordBatch ────────► Writer     │
├─────────────────────────────────────────────────────────┤
│                    Format Handlers                       │
│  ┌─────┐ ┌─────┐ ┌───────┐ ┌───────┐ ┌─────┐ ┌──────┐ │
│  │ CSV │ │JSON │ │Parquet│ │ Arrow │ │Excel│ │ Avro │ │
│  │ r/w │ │ r/w │ │  r/w  │ │  r/w  │ │ r/w │ │ r/w  │ │
│  └─────┘ └─────┘ └───────┘ └───────┘ └─────┘ └──────┘ │
│                              ┌─────┐                    │
│                              │ ORC │                    │
│                              │r/⚠️ │  (读仅)             │
│                              └─────┘                    │
├─────────────────────────────────────────────────────────┤
│                External Libraries                        │
│  arrow-rs │ calamine │ orc-rust │ apache-avro │ ...     │
└─────────────────────────────────────────────────────────┘
```

---

## 3. 核心 Trait 定义

```rust
// RecordBatchStream: 流式迭代器，支持逐批处理
pub type RecordBatchStream = Box<dyn Iterator<Item = Result<RecordBatch>>>;

// 每个格式实现此 trait 以支持读取
pub trait FormatReader: Send + Sync {
    fn read(&self, path: &Path, schema_override: Option<Schema>)
        -> Result<(Schema, RecordBatchStream)>;

    fn infer_schema(&self, path: &Path, lines: usize)
        -> Result<Schema>;

    fn format_name(&self) -> &'static str;
}

// 每个格式实现此 trait 以支持写入
pub trait FormatWriter: Send + Sync {
    fn write(&self, path: &Path, schema: &Schema,
             batches: RecordBatchStream) -> Result<()>;

    fn supports_streaming(&self) -> bool { true }

    fn format_name(&self) -> &'static str;
}
```

**设计要点：**
- `Send + Sync`：可用于线程池（未来扩展）
- `RecordBatchStream` 是 `Iterator` 而非 `Stream`：同步 IO，简单可靠
- `supports_streaming()` 默认 `true`：多数格式支持流式，Excel 覆写为 `false`

---

## 4. 数据流详解

```
                        ┌─────────────────┐
  data.csv ──────────► │   CsvReader      │
                        │    · 编码检测      │
                        │    · 压缩解压      │
                        │    · Schema推断   │
                        └────────┬────────┘
                                 │ RecordBatch(1024 rows) × N
                                 ▼
                        ┌─────────────────┐
                        │  SchemaAdapter  │  (按需)
                        │  typed schema →  │
                        │  string repr    │
                        └────────┬────────┘
                                 │ RecordBatch stream
                                 ▼
                 ┌───────────────────────────┐
                 │    ConversionEngine       │
                 │                           │
                 │  ┌───────────────────┐    │
                 │  │  PartitionWriter  │    │
                 │  │  · --partition-rows  │  │
                 │  │  · --partition-size  │  │
                 │  │  → output_000001.xlsx │ │
                 │  └───────────────────┘    │
                 │                           │
                 │  mode: stream / memory    │
                 └───────────┬───────────────┘
                             │ RecordBatch(s)
                             ▼
                        ┌─────────────────┐
  output.xlsx ◄──────── │  ExcelWriter    │
                        │  · 内存模式      │
                        │  · 不支持流式写入  │
                        └─────────────────┘
```

### 处理模式决策

```
--mode stream?  ──────────► 强制流式
--mode memory?  ──────────► 强制内存
自动检测:
  ├─ Writer 不支持流式? ──► 内存模式 + WARNING
  ├─ 文件 < 100MB ───────► 内存模式
  └─ 文件 ≥ 100MB ───────► 流式模式
```

---

## 5. 项目结构

```
ferryman/
├── Cargo.toml                   # workspace root + 二进制 crate
├── src/
│   ├── main.rs                  # ferryman 入口 → ferryman_core::run_cli()
│   └── bin/fm.rs                # fm 别名入口
│
├── crates/ferryman-core/        # ⬅ 核心库
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs               # 模块声明 + run_cli() + dispatch
│       ├── error.rs             # FerrymanError (11 variants)
│       ├── traits.rs            # FormatReader + FormatWriter
│       ├── schema.rs            # UserSchema, SchemaAdapter
│       ├── registry.rs          # FormatRegistry, detect_format()
│       ├── engine.rs            # ConversionEngine, ConvertConfig
│       ├── partition.rs         # PartitionWriter
│       ├── progress.rs          # ProgressTracker (indicatif)
│       ├── cli.rs               # clap 参数定义
│       └── formats/
│           ├── mod.rs
│           ├── csv.rs           # CsvReader + CsvWriter
│           ├── json.rs          # JsonReader + JsonWriter
│           ├── parquet.rs       # ParquetReader + ParquetWriter
│           ├── arrow_ipc.rs     # ArrowIpcReader + ArrowIpcWriter
│           ├── excel.rs         # ExcelReader + ExcelWriter
│           ├── orc.rs           # OrcReader (只读)
│           └── avro.rs          # AvroFormatReader + AvroFormatWriter
│
├── tests/
│   └── roundtrip_tests.rs       # 集成测试 (6 个场景)
│
├── test_data/
│   ├── generate.py              # 测试数据生成器
│   └── README.md
│
└── docs/
    └── superpowers/
        ├── specs/               # 设计规范
        └── plans/               # 实现计划
```

### 模块职责

| 模块 | 职责 | 依赖 |
|------|------|------|
| `error.rs` | 统一错误类型，含 `thiserror` + `From` 自动转换 | 无 |
| `traits.rs` | FormatReader / FormatWriter 接口定义 | error |
| `schema.rs` | JSON Schema 文件加载、类型字符串→Arrow类型映射 | error |
| `registry.rs` | 格式名→Handler 映射表、扩展名检测 | traits, error |
| `engine.rs` | 转换编排：读→适配→分区→写 | traits, schema, partition, error |
| `partition.rs` | 按行/大小切分输出文件 | 无 |
| `progress.rs` | indicatif 进度条封装 | 无 |
| `cli.rs` | clap derive 参数定义 | 无 |
| `formats/*.rs` | 各格式的 Reader + Writer 实现 | traits, error, schema |

---

## 6. 格式支持矩阵

| 格式 | 读取库 | 写入库 | 流式读 | 流式写 | 压缩 |
|------|--------|--------|--------|--------|------|
| CSV | arrow-csv | arrow-csv | ✅ | ✅ | gzip/bzip2/xz/zstd |
| JSON/JSONL | arrow-json | arrow-json | ✅ | ✅ | 文件级压缩 |
| Parquet | parquet (arrow-rs) | parquet (arrow-rs) | ✅ | ✅ | snappy/zstd/gzip/lz4 |
| Arrow IPC | arrow-ipc | arrow-ipc | ✅ | ✅ | lz4/zstd |
| Excel | calamine | rust_xlsxwriter | ✅ | ❌ | 内置 |
| ORC | orc-rust | ⚠️ 无纯Rust库 | ✅ | — | 内置 |
| Avro | apache-avro | apache-avro | ✅ | ✅ | 内置 |

### ORC 写入说明

Rust 生态无纯 Rust ORC Writer。当前：
- `ORC → 其他`：✅ 正常工作
- `其他 → ORC`：❌ 报错，引导用户使用 Parquet 或外部工具

---

## 7. Schema 策略

```
              源格式有 Schema?
              /              \
            是                否
            /                  \
   目标有 Schema?        目标有 Schema?
   /          \          /          \
  是          否        是          否
  │           │         │           │
  ▼           ▼         ▼           ▼
直接映射    类型→字符串  自动推断    透传字符串
类型冲突    +WARNING    (1000行)   (所有列 Utf8)
--lax 降级             或--schema
```

### Schema 文件格式

```json
{
  "columns": [
    {"name": "id",   "type": "Int64"},
    {"name": "name", "type": "Utf8"},
    {"name": "age",  "type": "Int32"}
  ]
}
```

支持类型：`Int8`-`Int64`, `UInt8`-`UInt64`, `Float32`, `Float64`, `Utf8`, `LargeUtf8`, `Bool`, `Date32`, `Date64`, `Timestamp(s/ms/us/ns)`, `Binary`, `LargeBinary`

---

## 8. 嵌套数据处理

Flat 目标格式 (CSV/Excel) 无法表达嵌套结构 → 默认将嵌套值序列化为 JSON 字符串：

```
{"user": {"name": "Bob", "addr": {"city": "NY"}}}
                         ↓
CSV: user,"{""name"":""Bob"",""addr"":{""city"":""NY""}}"
```

反向转换时自动检测 JSON 字符串列并还原为嵌套结构。

---

## 9. 错误处理模式

| 模式 | 标志 | 行为 |
|------|------|------|
| **strict** (默认) | — | 任何格式错误立即终止，打印行号和详情 |
| **lax** | `--lax` | 跳过错误行，缺失字段填 NULL，stderr 输出 WARNING |

```
WARNING: Row 1523 skipped: missing column 'email'
WARNING: Type downgrade: Int64 → Utf8 for column 'age' (target: CSV)
WARNING: Switching to memory mode: Excel writer does not support streaming
```

---

## 10. 依赖关系

```
ferryman (二进制)
  ├── ferryman-core (核心库)
  │   ├── arrow (RecordBatch, Schema, Array)
  │   ├── arrow-csv (CSV Reader/Writer)
  │   ├── arrow-json (JSON Reader/Writer)
  │   ├── parquet (Parquet Reader/Writer + 压缩)
  │   ├── arrow-ipc (Arrow IPC/Feather)
  │   ├── calamine (Excel Reader)
  │   ├── rust_xlsxwriter (Excel Writer)
  │   ├── orc-rust (ORC Reader)
  │   ├── apache-avro (Avro Reader/Writer)
  │   ├── serde / serde_json (Schema JSON)
  │   ├── thiserror (错误类型)
  │   ├── indicatif (进度条)
  │   ├── flate2 / bzip2 / xz2 / zstd (压缩)
  │   └── chardetng (编码检测)
  ├── clap (CLI 参数解析)
  └── env_logger (日志)
```

---

## 11. 新增格式指南

添加新格式只需 4 步：

**Step 1** — 在 `crates/ferryman-core/src/formats/` 创建文件：

```rust
// formats/newfmt.rs
pub struct NewFmtReader;
impl FormatReader for NewFmtReader { /* ... */ }

pub struct NewFmtWriter;
impl FormatWriter for NewFmtWriter { /* ... */ }
```

**Step 2** — 注册到 `formats/mod.rs`：

```rust
pub mod newfmt;
```

**Step 3** — 注册到 `lib.rs` 的 `register_all_formats()`：

```rust
reg.register_reader("newfmt", || Box::new(formats::newfmt::NewFmtReader));
reg.register_writer("newfmt", || Box::new(formats::newfmt::NewFmtWriter));
```

**Step 4** — 在 `registry.rs` 的 `EXTENSION_MAP` 添加扩展名映射：

```rust
("newfmt", "newfmt"),
```

新增格式自动与所有已有格式互通，无需额外配置。

---

## 12. 测试策略

```
┌─────────────────────┐
│  单元测试 (lib)       │  每个模块独立测试
│  30 tests            │  - Schema 解析
│                      │  - 格式检测
├─────────────────────┤  - 类型映射
│  格式 Handler 测试   │  - 分区逻辑
│  每个格式 2-4 tests  │
│                      │
├─────────────────────┤
│  集成测试             │  端到端 CLI 测试
│  6 tests             │  - CSV→JSON 往返
│                      │  - list-formats
│                      │  - detect 命令
│                      │  - ORC 写入错误
│                      │  - no-clobber
│                      │  - 格式自动检测
└─────────────────────┘
```

---

## 13. 关键设计决策记录

| 决策 | 理由 |
|------|------|
| Arrow 而非 Polars | 更灵活的格式适配，编译体积可控 |
| 同步 Iterator 而非 async | CLI 工具无需异步，简化错误处理 |
| 单一 workspace crate | 7 个格式 Handler 各约 100 行，拆 crate 过度工程 |
| 默认流式模式 | 保证 TB 级数据可处理 |
| 嵌套→JSON 字符串 | 不丢数据，反向可还原 |
| ORC 只读 | Rust 生态无纯 Rust Writer；不引入 C++ 依赖破坏轻量定位 |
