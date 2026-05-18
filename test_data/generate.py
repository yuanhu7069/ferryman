#!/usr/bin/env python3
"""
Test data generator for Ferryman CLI.
Generates CSV, JSON, JSONL, Parquet, Arrow, Excel, ORC, Avro files
with configurable columns, row count, and data types.

Usage:
    python test_data/generate.py --fmt csv --rows 10000 --cols "id:int,name:str,score:float"
    python test_data/generate.py --fmt parquet --rows 1000000 --preset wide
    python test_data/generate.py --fmt json  --rows 500  --preset nested

Dependencies:
    pip install pyarrow pandas openpyxl fastavro
"""

import argparse
import os
import random
import string
import sys
import time
from datetime import datetime, timedelta
from pathlib import Path

# ---------------------------------------------------------------------------
# Column type definitions
# ---------------------------------------------------------------------------
TYPE_HANDLERS = {}


def register(name):
    def deco(fn):
        TYPE_HANDLERS[name] = fn
        return fn
    return deco


@register("int")
def gen_int(rng):
    return rng.randint(1, 99_999_999)


@register("int8")
def gen_int8(rng):
    return rng.randint(-128, 127)


@register("int16")
def gen_int16(rng):
    return rng.randint(-32768, 32767)


@register("int32")
def gen_int32(rng):
    return rng.randint(-2_147_483_648, 2_147_483_647)


@register("int64")
def gen_int64(rng):
    return rng.randint(-9_223_372_036_854_775_808, 9_223_372_036_854_775_807)


@register("float")
def gen_float(rng):
    return round(rng.uniform(-1000.0, 1000.0), 4)


@register("float64")
def gen_float64(rng):
    return rng.uniform(-1e308, 1e308)


@register("bool")
def gen_bool(rng):
    return rng.choice([True, False])


@register("str")
def gen_str(rng):
    length = rng.randint(3, 20)
    return ''.join(rng.choices(string.ascii_letters, k=length))


@register("name")
def gen_name(rng):
    first = rng.choice(["Alice","Bob","Charlie","Diana","Eve","Frank","Grace",
                         "Henry","Iris","Jack","Kate","Leo","Mia","Noah","Olivia"])
    last = rng.choice(["Smith","Johnson","Williams","Brown","Jones","Garcia",
                        "Miller","Davis","Rodriguez","Martinez"])
    return f"{first} {last}"


@register("email")
def gen_email(rng):
    user = ''.join(rng.choices(string.ascii_lowercase, k=rng.randint(5, 10)))
    domain = rng.choice(["example.com","test.org","mail.io","demo.net"])
    return f"{user}@{domain}"


@register("date")
def gen_date(rng):
    start = datetime(2020, 1, 1)
    delta = timedelta(days=rng.randint(0, 2000))
    return (start + delta).strftime("%Y-%m-%d")


@register("datetime")
def gen_datetime(rng):
    start = datetime(2020, 1, 1, 0, 0, 0)
    delta = timedelta(seconds=rng.randint(0, 2000 * 86400))
    return (start + delta).isoformat()


@register("category")
def gen_category(rng):
    return rng.choice(["A","B","C","D","E"])


@register("uuid")
def gen_uuid(rng):
    import uuid
    return str(uuid.UUID(int=rng.getrandbits(128)))


@register("json_str")
def gen_json_str(rng):
    """Nested JSON string for testing nested data serialization."""
    return '{"street":"%d Main St","city":"%s","zip":"%05d"}' % (
        rng.randint(1, 9999),
        rng.choice(["NYC","LA","Chicago","Houston","Phoenix"]),
        rng.randint(10000, 99999),
    )


@register("array")
def gen_array(rng):
    """Array for nested data testing."""
    return [rng.randint(1, 100) for _ in range(rng.randint(1, 5))]


@register("null_10pct")
def gen_null(rng):
    """10% chance of None for NULL testing."""
    return None if rng.random() < 0.1 else rng.randint(1, 9999)


# ---------------------------------------------------------------------------
# Preset column sets
# ---------------------------------------------------------------------------
PRESETS = {
    "simple": [
        ("id", "int"),
        ("name", "name"),
        ("score", "float"),
    ],
    "wide": [
        ("id", "int"),
        ("name", "name"),
        ("email", "email"),
        ("age", "int"),
        ("salary", "float"),
        ("department", "category"),
        ("hire_date", "date"),
        ("is_manager", "bool"),
        ("uuid", "uuid"),
        ("score_a", "float"),
        ("score_b", "float"),
        ("score_c", "float"),
        ("notes", "str"),
        ("region", "category"),
        ("level", "int"),
        ("active", "bool"),
        ("last_login", "datetime"),
        ("team_size", "int"),
        ("budget", "float"),
        ("performance", "category"),
    ],
    "nested": [
        ("id", "int"),
        ("name", "name"),
        ("address", "json_str"),
        ("tags", "array"),
    ],
    "nulls": [
        ("id", "int"),
        ("name", "name"),
        ("optional_field", "null_10pct"),
        ("score", "float"),
    ],
    "types": [
        ("col_int8",   "int8"),
        ("col_int16",  "int16"),
        ("col_int32",  "int32"),
        ("col_int64",  "int64"),
        ("col_float",  "float"),
        ("col_float64","float64"),
        ("col_bool",   "bool"),
        ("col_str",    "str"),
        ("col_date",   "date"),
        ("col_dt",     "datetime"),
        ("col_cat",    "category"),
    ],
}


# ---------------------------------------------------------------------------
# Data generation
# ---------------------------------------------------------------------------
def parse_columns(cols_str: str):
    """Parse 'name:type,name:type' into list of (name, type) tuples."""
    columns = []
    for part in cols_str.split(","):
        part = part.strip()
        if ":" in part:
            name, dtype = part.split(":", 1)
            if dtype not in TYPE_HANDLERS:
                print(f"WARNING: unknown type '{dtype}', falling back to 'str'")
                dtype = "str"
            columns.append((name.strip(), dtype.strip()))
        else:
            columns.append((part.strip(), "str"))
    return columns


def generate_rows(columns, n_rows: int, seed: int = 42):
    """Yield dicts, one per row."""
    rng = random.Random(seed)
    col_names = [c[0] for c in columns]
    generators = [(c[0], TYPE_HANDLERS[c[1]]) for c in columns]

    for _ in range(n_rows):
        row = {}
        for name, gen_fn in generators:
            row[name] = gen_fn(rng)
        yield row


def write_csv(rows, columns, path, n_rows):
    import csv
    col_names = [c[0] for c in columns]
    with open(path, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(col_names)
        for row in rows:
            w.writerow([row[name] for name in col_names])
    print_size(path, n_rows)


def write_json(rows, columns, path, n_rows):
    import json
    data = list(rows)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, default=str)
    print_size(path, n_rows)


def write_jsonl(rows, columns, path, n_rows):
    import json
    with open(path, "w", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row, default=str) + "\n")
    print_size(path, n_rows)


def write_parquet(rows, columns, path, n_rows):
    try:
        import pyarrow as pa
        import pyarrow.parquet as pq
    except ImportError:
        print("  SKIP parquet: pyarrow not installed (pip install pyarrow)")
        return
    table = rows_to_arrow(rows, columns, n_rows)
    pq.write_table(table, path, compression="snappy")
    print_size(path, n_rows)


def write_arrow(rows, columns, path, n_rows):
    try:
        import pyarrow as pa
        import pyarrow.ipc as ipc
    except ImportError:
        print("  SKIP arrow: pyarrow not installed (pip install pyarrow)")
        return
    table = rows_to_arrow(rows, columns, n_rows)
    with pa.OSFile(str(path), "wb") as f:
        with ipc.new_file(f, table.schema) as writer:
            writer.write_table(table)
    print_size(path, n_rows)


def write_excel(rows, columns, path, n_rows):
    try:
        import openpyxl
    except ImportError:
        print("  SKIP excel: openpyxl not installed (pip install openpyxl)")
        return
    wb = openpyxl.Workbook()
    ws = wb.active
    col_names = [c[0] for c in columns]
    ws.append(col_names)
    for row in rows:
        ws.append([row[name] for name in col_names])
    wb.save(path)
    print_size(path, n_rows)


def write_orc(rows, columns, path, n_rows):
    try:
        import pyarrow.orc as orc
    except ImportError:
        print("  SKIP orc: pyarrow not installed (pip install pyarrow)")
        return
    table = rows_to_arrow(rows, columns, n_rows)
    orc.write_table(table, path)
    print_size(path, n_rows)


def write_avro(rows, columns, path, n_rows):
    try:
        import fastavro
        from fastavro.schema import make_avro_record_schema, make_field_schema
    except ImportError:
        print("  SKIP avro: fastavro not installed (pip install fastavro)")
        return

    col_names = [c[0] for c in columns]
    type_map = {
        "int": "long", "int8": "int", "int16": "int", "int32": "int",
        "int64": "long", "float": "double", "float64": "double",
        "bool": "boolean", "str": "string", "name": "string",
        "email": "string", "date": "string", "datetime": "string",
        "category": "string", "uuid": "string", "json_str": "string",
        "null_10pct": ["null", "long"],
    }

    fields = []
    for name, dtype in columns:
        avro_type = type_map.get(dtype, "string")
        if isinstance(avro_type, list):
            avro_type = avro_type[1]
        fields.append({"name": name, "type": avro_type})

    schema = {
        "type": "record",
        "name": "FerrymanTestData",
        "fields": fields,
    }
    parsed = fastavro.parse_schema(schema)

    records = []
    for row in rows:
        record = {}
        for name in col_names:
            val = row.get(name)
            record[name] = val
        records.append(record)

    with open(path, "wb") as f:
        fastavro.writer(f, parsed, records)
    print_size(path, n_rows)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
def rows_to_arrow(rows, columns, n_rows):
    """Build PyArrow Table from generator, auto-inferring types."""
    import pyarrow as pa
    data = list(rows)
    col_names = [c[0] for c in columns]
    arrays = {}
    for name in col_names:
        values = [row[name] for row in data]
        try:
            arrays[name] = pa.array(values)
        except (pa.ArrowInvalid, pa.ArrowTypeError, TypeError):
            arrays[name] = pa.array([str(v) for v in values])
    return pa.table(arrays)


def print_size(path, n_rows):
    size = os.path.getsize(path)
    if size >= 1_000_000_000:
        size_str = f"{size/1_000_000_000:.2f} GB"
    elif size >= 1_000_000:
        size_str = f"{size/1_000_000:.2f} MB"
    elif size >= 1_000:
        size_str = f"{size/1_000:.2f} KB"
    else:
        size_str = f"{size} B"
    print(f"  Generated: {path}  ({n_rows:,} rows, {size_str})")


WRITERS = {
    "csv":     write_csv,
    "json":    write_json,
    "jsonl":   write_jsonl,
    "parquet": write_parquet,
    "arrow":   write_arrow,
    "feather": write_arrow,
    "excel":   write_excel,
    "xlsx":    write_excel,
    "orc":     write_orc,
    "avro":    write_avro,
}


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
def main():
    parser = argparse.ArgumentParser(
        description="Generate test data files for Ferryman CLI",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s --fmt csv     --rows 10000
  %(prog)s --fmt parquet --rows 1000000 --preset wide
  %(prog)s --fmt json    --rows 500  --preset nested
  %(prog)s --fmt all     --rows 10000 --preset simple
  %(prog)s --fmt csv,parquet,json --rows 50000 --cols "id:int,name:str,score:float"
        """,
    )
    parser.add_argument("--fmt", default="csv",
                        help="Output format(s): csv, json, jsonl, parquet, arrow, excel, orc, avro, "
                             "or 'all' for every format. Comma-separated for multiple. (default: csv)")
    parser.add_argument("--rows", type=int, default=1000,
                        help="Number of rows to generate (default: 1000)")
    parser.add_argument("--cols", default="",
                        help="Column definitions: name:type,name:type (e.g., 'id:int,name:str,score:float')")
    parser.add_argument("--preset", default="simple",
                        choices=list(PRESETS.keys()),
                        help=f"Column preset (default: simple). Options: {', '.join(PRESETS.keys())}")
    parser.add_argument("--seed", type=int, default=42,
                        help="Random seed for reproducibility (default: 42)")
    parser.add_argument("-o", "--output", default="",
                        help="Output path prefix (default: test_data/data)")
    parser.add_argument("--dir", default="test_data",
                        help="Output directory (default: test_data)")

    args = parser.parse_args()

    # Resolve format list
    if args.fmt == "all":
        formats = ["csv", "json", "jsonl", "parquet", "arrow", "excel", "orc", "avro"]
    else:
        formats = [f.strip() for f in args.fmt.split(",")]

    for f in formats:
        if f not in WRITERS and f != "all":
            print(f"ERROR: unknown format '{f}'. Supported: {', '.join(WRITERS.keys())}")
            sys.exit(1)

    # Resolve columns
    if args.cols:
        columns = parse_columns(args.cols)
    else:
        columns = PRESETS[args.preset]

    # Output directory
    out_dir = Path(args.dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    prefix = args.output or f"{out_dir}/data"

    # Print summary
    print(f"\n{'='*60}")
    print(f"  Ferryman Test Data Generator")
    print(f"  Rows: {args.rows:,}  |  Seed: {args.seed}")
    print(f"  Columns ({len(columns)}): {', '.join(c[0]+':'+c[1] for c in columns)}")
    print(f"  Formats: {', '.join(formats)}")
    print(f"{'='*60}\n")

    # Generate
    for fmt in formats:
        writer = WRITERS.get(fmt)
        if not writer:
            print(f"SKIP: {fmt} (no writer available)")
            continue

        path = f"{prefix}.{fmt}"
        if fmt == "xlsx":
            path = f"{prefix}.xlsx"
        elif fmt == "feather":
            path = f"{prefix}.feather"

        t0 = time.time()
        rows = generate_rows(columns, args.rows, args.seed)
        writer(rows, columns, path, args.rows)
        elapsed = time.time() - t0
        print(f"  Time: {elapsed:.2f}s\n")


if __name__ == "__main__":
    main()
