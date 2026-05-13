use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ferryman", about = "Data format converter", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Convert between data formats
    Convert(ConvertArgs),
    /// List all supported formats
    ListFormats,
    /// Detect the format of a file
    Detect(DetectArgs),
}

#[derive(Args)]
pub struct ConvertArgs {
    #[arg(short = 'f', long)]
    pub from: Option<String>,

    #[arg(short = 't', long)]
    pub to: Option<String>,

    pub input: PathBuf,
    pub output: PathBuf,

    #[arg(long)]
    pub schema: Option<PathBuf>,

    #[arg(long, default_value = "1000")]
    pub infer_schema_lines: usize,

    #[arg(long)]
    pub mode: Option<String>,

    #[arg(long)]
    pub lax: bool,

    #[arg(short = 'y', long)]
    pub force: bool,

    #[arg(long, conflicts_with = "force")]
    pub no_clobber: bool,

    #[arg(long)]
    pub quiet: bool,

    #[arg(long)]
    pub partition_rows: Option<usize>,

    #[arg(long, value_parser = parse_partition_size)]
    pub partition_size: Option<u64>,

    #[arg(long)]
    pub compress: Option<String>,

    #[arg(long)]
    pub compress_level: Option<u32>,

    #[arg(long, default_value = ",")]
    pub delimiter: String,

    #[arg(long, default_value = "true")]
    pub has_header: bool,

    #[arg(long, value_delimiter = ',')]
    pub null_values: Option<Vec<String>>,

    #[arg(long)]
    pub null_repr: Option<String>,

    #[arg(long)]
    pub json_lines: bool,

    #[arg(long)]
    pub sheet: Option<String>,

    #[arg(long)]
    pub encoding: Option<String>,
}

#[derive(Args)]
pub struct DetectArgs {
    pub file: PathBuf,
}

fn parse_partition_size(s: &str) -> Result<u64, String> {
    let s_upper = s.trim().to_uppercase();
    let (num_str, multiplier) = if s_upper.ends_with("GB") || s_upper.ends_with('G') {
        (s_upper[..s_upper.len()-2].trim_end_matches('G'), 1_000_000_000u64)
    } else if s_upper.ends_with("MB") || s_upper.ends_with('M') {
        (s_upper[..s_upper.len()-2].trim_end_matches('M'), 1_000_000)
    } else if s_upper.ends_with("KB") || s_upper.ends_with('K') {
        (s_upper[..s_upper.len()-2].trim_end_matches('K'), 1_000)
    } else {
        (s_upper.as_str(), 1)
    };
    let num: u64 = num_str.trim().parse().map_err(|_| format!("Invalid size: {}", s))?;
    Ok(num * multiplier)
}
