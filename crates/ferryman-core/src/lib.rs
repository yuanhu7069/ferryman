pub mod cli;
pub mod engine;
pub mod error;
pub mod formats;
pub mod partition;
pub mod progress;
pub mod registry;
pub mod schema;
pub mod traits;

use crate::error::Result;
use crate::registry::FormatRegistry;

pub fn run_cli() {
    use clap::Parser;
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Commands::Convert(args) => {
            if let Err(e) = execute_convert(args) {
                eprintln!("ERROR: {}", e);
                std::process::exit(1);
            }
        }
        cli::Commands::ListFormats => {
            let mut reg = FormatRegistry::new();
            register_all_formats(&mut reg);
            println!("Supported formats:");
            for fmt in reg.list_formats() {
                let read = if reg.supports_read(&fmt) { "read" } else { "—" };
                let write = if reg.supports_write(&fmt) { "write" } else { "—" };
                println!("  {:12}  {}  {}", fmt, read, write);
            }
        }
        cli::Commands::Detect(detect_args) => {
            match crate::registry::detect_format(&detect_args.file) {
                Ok(format) => println!("{}", format),
                Err(e) => {
                    eprintln!("ERROR: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn execute_convert(args: cli::ConvertArgs) -> Result<()> {
    let from_format = match &args.from {
        Some(f) => f.clone(),
        None => crate::registry::detect_format(&args.input)?.to_string(),
    };
    let to_format = match &args.to {
        Some(f) => f.clone(),
        None => crate::registry::detect_format(&args.output)?.to_string(),
    };

    if args.output.exists() && !args.force {
        if args.no_clobber {
            return Err(crate::error::FerrymanError::FileExists(args.output.display().to_string()));
        }
        use std::io::{Write, stdin, stdout};
        eprint!("WARNING: {} already exists. Overwrite? [y/N]: ", args.output.display());
        stdout().flush().unwrap();
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let mut reg = FormatRegistry::new();
    register_all_formats(&mut reg);

    let reader = build_reader(&reg, &from_format, &args)?;
    let writer = build_writer(&reg, &to_format, &args)?;

    let delimiter_byte = args.delimiter.as_bytes().first().copied().unwrap_or(b',');
    let config = crate::engine::ConvertConfig {
        input: args.input.clone(),
        output: args.output.clone(),
        from_format,
        to_format,
        schema_file: args.schema,
        infer_schema_lines: args.infer_schema_lines,
        mode: args.mode,
        lax: args.lax,
        force: args.force,
        no_clobber: args.no_clobber,
        quiet: args.quiet,
        partition_rows: args.partition_rows,
        partition_size: args.partition_size,
        compression: args.compress,
        csv_delimiter: delimiter_byte,
        csv_has_header: args.has_header,
        csv_null_values: args.null_values.unwrap_or_else(|| vec![String::new()]),
        csv_null_repr: args.null_repr.unwrap_or_default(),
        json_lines: args.json_lines,
        excel_sheet: args.sheet,
        encoding: args.encoding,
    };

    let engine = crate::engine::ConversionEngine::new(reader, writer);
    engine.convert(&config)
}

fn register_all_formats(reg: &mut FormatRegistry) {
    use crate::formats::*;
    reg.register_reader("csv", || Box::new(csv::CsvReader::default()));
    reg.register_writer("csv", || Box::new(csv::CsvWriter::default()));
    reg.register_reader("json", || Box::new(json::JsonReader));
    reg.register_writer("json", || Box::new(json::JsonWriter));
    reg.register_reader("parquet", || Box::new(parquet::ParquetReader));
    reg.register_writer("parquet", || Box::new(parquet::ParquetWriter::default()));
    reg.register_reader("arrow", || Box::new(arrow_ipc::ArrowIpcReader));
    reg.register_writer("arrow", || Box::new(arrow_ipc::ArrowIpcWriter));
    reg.register_reader("excel", || Box::new(excel::ExcelReader::default()));
    reg.register_writer("excel", || Box::new(excel::ExcelWriter));
    reg.register_reader("orc", || Box::new(orc::OrcReader));
    reg.register_writer("orc", || Box::new(orc::OrcWriter));
    reg.register_reader("avro", || Box::new(avro::AvroFormatReader));
    reg.register_writer("avro", || Box::new(avro::AvroFormatWriter));
}

fn build_reader(reg: &FormatRegistry, format: &str, args: &cli::ConvertArgs) -> Result<Box<dyn crate::traits::FormatReader>> {
    use crate::formats::*;
    match format {
        "csv" => Ok(Box::new(csv::CsvReader {
            delimiter: args.delimiter.as_bytes().first().copied().unwrap_or(b','),
            has_header: args.has_header,
            null_values: args.null_values.clone().unwrap_or_else(|| vec![String::new()]),
        })),
        "excel" => Ok(Box::new(excel::ExcelReader { sheet_name: args.sheet.clone() })),
        _ => reg.get_reader(format),
    }
}

fn build_writer(reg: &FormatRegistry, format: &str, args: &cli::ConvertArgs) -> Result<Box<dyn crate::traits::FormatWriter>> {
    use crate::formats::*;
    match format {
        "csv" => Ok(Box::new(csv::CsvWriter {
            delimiter: args.delimiter.as_bytes().first().copied().unwrap_or(b','),
            null_repr: args.null_repr.clone().unwrap_or_default(),
        })),
        "parquet" => {
            let comp = args.compress.as_deref()
                .map(parquet::compression_from_str)
                .unwrap_or(Ok(::parquet::basic::Compression::SNAPPY))?;
            Ok(Box::new(parquet::ParquetWriter { compression: comp }))
        }
        _ => reg.get_writer(format),
    }
}
