# Ferryman 🚣

> 数据的摆渡人 — Lightweight CLI data format converter

Convert between 7 data formats with a single command.

## Quick Start

```bash
# Install
cargo install ferryman

# Usage
fm convert -f csv -t parquet data.csv output.parquet
fm convert -f json -t csv data.json output.csv
fm convert -f excel -t parquet data.xlsx output.parquet
```

## Supported Formats

| Format | Read | Write | Notes |
|--------|------|-------|-------|
| CSV    | ✅   | ✅    | Custom delimiter, encoding, NULL handling |
| JSON   | ✅   | ✅    | JSONL and array-of-objects |
| Excel  | ✅   | ✅    | .xlsx only, sheet selection |
| Parquet| ✅   | ✅    | Column compression (snappy, zstd, gzip, lz4) |
| Arrow  | ✅   | ✅    | IPC/Feather format |
| ORC    | ✅   | ⚠️    | Read only (pending pure-Rust write support) |
| Avro   | ✅   | ✅    | |

## Options

```
fm convert [OPTIONS] <INPUT> <OUTPUT>

-f, --from <FMT>        Source format (auto-detect by default)
-t, --to <FMT>          Target format (auto-detect by default)
--schema <FILE>         Schema file (JSON)
--mode <memory|stream>  Processing mode
--lax                   Skip errors, continue processing
--partition-rows <N>    Split output every N rows
--partition-size <SIZE> Split output at file size (e.g., 500M)
--compress <FMT>        Output compression
--encoding <ENC>        Input file encoding
--null-values <LIST>    CSV values treated as NULL
--null-repr <STR>       NULL representation in CSV output
--force, -y             Overwrite existing files
--no-clobber            Error if file exists
--quiet                 No progress display

# Other commands
fm list-formats         List supported formats
fm detect <FILE>        Detect file format
```

## Build from Source

```bash
git clone https://github.com/xxx/ferryman
cd ferryman
cargo build --release
./target/release/ferryman --version
```
