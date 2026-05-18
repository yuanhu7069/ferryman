# test_data — Ferryman 测试数据生成器

## 安装

```bash
cd test_data
uv sync
```

## 快速开始

```bash
# 生成 1 万行 CSV
uv run python generate.py --fmt csv --rows 10000

# 查看生成的文件
ls -lh test_data/
```

## 常用场景

### 1. 基本转换测试

```bash
# 生成小文件，测试全部格式
uv run python generate.py --fmt all --rows 100 --preset simple

# 用 ferryman 互转验证
fm convert -f csv    -t parquet test_data/data.csv    test_data/out.parquet
fm convert -f json   -t csv     test_data/data.json   test_data/out.csv
fm convert -f parquet -t avro   test_data/data.parquet test_data/out.avro
fm convert -f avro   -t json    test_data/data.avro   test_data/out.json
fm convert -f arrow  -t excel   test_data/data.arrow  test_data/out.xlsx
fm convert -f excel  -t csv     test_data/data.xlsx   test_data/out.csv
```

### 2. 大文件流式测试

```bash
# 生成 100 万行 Parquet
uv run python generate.py --fmt parquet --rows 1000000 --preset wide
# 转 CSV，测试流式 + 分片
fm convert -f parquet -t csv test_data/data.parquet test_data/out.csv \
  --mode stream --partition-rows 200000
```

### 3. 嵌套数据处理

```bash
# JSON 嵌套对象 → CSV
uv run python generate.py --fmt json --rows 500 --preset nested
fm convert -f json -t csv test_data/data.json test_data/out.csv
```

### 4. NULL 值处理

```bash
uv run python generate.py --fmt csv --rows 1000 --preset nulls
fm convert -f csv -t parquet test_data/data.csv test_data/out.parquet \
  --null-values "NA,NULL,None"
```

### 5. 压缩测试

```bash
uv run python generate.py --fmt csv --rows 100000 --preset wide
# 输出 gzip 压缩
fm convert -f csv -t json test_data/data.csv test_data/out.json --compress gzip
# 输出 zstd 压缩的 Parquet
fm convert -f csv -t parquet test_data/data.csv test_data/out.parquet --compress zstd
```

### 6. Schema 测试

```bash
uv run python generate.py --fmt csv --rows 1000 --preset types
# 生成 schema 文件，手动指定类型
cat > test_data/my_schema.json << 'EOF'
{"columns": [{"name":"col_int64","type":"Int64"},{"name":"col_str","type":"Utf8"}]}
EOF
fm convert -f csv -t parquet test_data/data.csv test_data/out.parquet --schema test_data/my_schema.json
```

### 7. ORC 读取测试（写暂不支持）

```bash
uv run python generate.py --fmt orc --rows 1000
fm convert -f orc -t csv test_data/data.orc test_data/out.csv     # ✅
fm convert -f csv -t orc test_data/data.csv test_data/out.orc     # ❌ 报错提示
```

### 8. 编码测试

```bash
uv run python generate.py --fmt csv --rows 1000 --preset simple
fm convert -f csv -t json test_data/data.csv test_data/out.json --encoding utf-8
```

## 命令行参考

```
uv run python generate.py [OPTIONS]

  --fmt FORMAT      输出格式：csv, json, jsonl, parquet, arrow, excel, orc, avro
                    多个用逗号分隔，或 all 生成全部 (默认: csv)

  --rows N          生成行数 (默认: 1000)

  --preset NAME     列预设 (默认: simple)
                    simple  - 3列 (id, name, score)
                    wide    - 20列 (全类型混合)
                    nested  - 4列 (含 JSON 嵌套对象和数组)
                    nulls   - 4列 (10% NULL 值)
                    types   - 11列 (覆盖所有箭头类型)

  --cols SPEC       自定义列：name:type,name:type
                    例: --cols "id:int,name:str,age:int,active:bool"

  --seed N          随机种子，保证可复现 (默认: 42)

  -o, --output PREFIX  输出文件前缀 (默认: test_data/data)

  --dir DIR         输出目录 (默认: test_data)
```

## 数据类型参考

| 类型 | 说明 | 示例 |
|------|------|------|
| `int` | 通用整数 | 1337 |
| `int8/16/32/64` | 指定位宽整数 | -128 ~ 127 |
| `float` | 浮点数 | 3.14 |
| `float64` | 双精度 | 1e308 |
| `bool` | 布尔 | true |
| `str` | 随机字符串 | aBcDeF |
| `name` | 人名 | Alice Smith |
| `email` | 邮箱 | user@test.org |
| `date` | 日期 | 2023-06-15 |
| `datetime` | 日期时间 | 2023-06-15T08:30:00 |
| `category` | 枚举 | A/B/C/D/E |
| `uuid` | UUID | 550e8400-... |
| `json_str` | 嵌套JSON字符串 | {"street":"123"} |
| `array` | 数组 | [1, 5, 42] |
| `null_10pct` | 90%有值 10%NULL | NULL 测试 |
