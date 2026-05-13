use thiserror::Error;

#[derive(Error, Debug)]
pub enum FerrymanError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    #[error("JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Format not supported: {0}")]
    UnsupportedFormat(String),

    #[error("ORC write is not supported. Workarounds:\n\
             1. Convert to Parquet: fm convert -f {src} -t parquet {input} output.parquet\n\
             2. Use external tools: orc-tools, datu")]
    OrcWriteNotSupported { src: String, input: String },

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("Conversion error at row {row}: {message}")]
    ConversionError { row: usize, message: String },

    #[error("File already exists: {0}. Use --force to overwrite.")]
    FileExists(String),

    #[error("Encoding detection failed: {0}")]
    EncodingDetection(String),

    #[error("{0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, FerrymanError>;
