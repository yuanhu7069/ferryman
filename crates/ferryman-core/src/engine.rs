use std::path::{Path, PathBuf};
use arrow::datatypes::Schema;
use arrow::record_batch::RecordBatch;
use crate::error::Result;
use crate::traits::{FormatReader, FormatWriter, RecordBatchStream};
use crate::schema::SchemaAdapter;

pub struct ConvertConfig {
    pub input: PathBuf,
    pub output: PathBuf,
    pub from_format: String,
    pub to_format: String,
    pub schema_file: Option<PathBuf>,
    pub infer_schema_lines: usize,
    pub mode: Option<String>,
    pub lax: bool,
    pub force: bool,
    pub no_clobber: bool,
    pub quiet: bool,
    pub partition_rows: Option<usize>,
    pub partition_size: Option<u64>,
    pub compression: Option<String>,
    pub csv_delimiter: u8,
    pub csv_has_header: bool,
    pub csv_null_values: Vec<String>,
    pub csv_null_repr: String,
    pub json_lines: bool,
    pub excel_sheet: Option<String>,
    pub encoding: Option<String>,
}

pub struct ConversionEngine {
    reader: Box<dyn FormatReader>,
    writer: Box<dyn FormatWriter>,
}

impl ConversionEngine {
    pub fn new(reader: Box<dyn FormatReader>, writer: Box<dyn FormatWriter>) -> Self {
        ConversionEngine { reader, writer }
    }

    pub fn convert(&self, config: &ConvertConfig) -> Result<()> {
        let use_streaming = self.detect_mode(config);
        let schema_override = if let Some(ref schema_path) = config.schema_file {
            let user_schema = crate::schema::UserSchema::from_file(schema_path)?;
            Some(user_schema.to_arrow_schema()?)
        } else {
            None
        };

        let (source_schema, batches) = self.reader.read(&config.input, schema_override)?;
        let target_has_schema = matches!(self.writer.format_name(), "parquet" | "arrow" | "avro");

        if SchemaAdapter::has_typed_schema(&source_schema) && !target_has_schema {
            eprintln!("WARNING: Type downgrade — typed source schema will be converted to strings for target format");
        }

        self.write_output(config, &source_schema, batches, use_streaming)
    }

    fn detect_mode(&self, config: &ConvertConfig) -> bool {
        if let Some(ref mode) = config.mode {
            return mode.as_str() == "stream";
        }
        if !self.writer.supports_streaming() {
            eprintln!("WARNING: {} writer does not support streaming, switching to memory mode", self.writer.format_name());
            return false;
        }
        if let Ok(meta) = std::fs::metadata(&config.input) {
            if meta.len() < 100 * 1024 * 1024 {
                return false; // < 100MB: memory mode
            }
        }
        true // default to streaming for large files
    }

    fn write_output(&self, config: &ConvertConfig, schema: &Schema, batches: RecordBatchStream, use_streaming: bool) -> Result<()> {
        let has_partitioning = config.partition_rows.is_some() || config.partition_size.is_some();

        if !has_partitioning {
            self.write_single(&config.output, schema, batches, use_streaming)
        } else {
            self.write_partitioned(config, schema, batches, use_streaming)
        }
    }

    fn write_single(&self, output: &Path, schema: &Schema, batches: RecordBatchStream, use_streaming: bool) -> Result<()> {
        if use_streaming && self.writer.supports_streaming() {
            self.writer.write(output, schema, batches)
        } else {
            let collected: Vec<RecordBatch> = batches.collect::<std::result::Result<Vec<_>, _>>()?;
            let stream: RecordBatchStream = Box::new(collected.into_iter().map(Ok));
            self.writer.write(output, schema, stream)
        }
    }

    fn write_partitioned(&self, config: &ConvertConfig, schema: &Schema, batches: RecordBatchStream, _use_streaming: bool) -> Result<()> {
        use crate::partition::PartitionWriter;

        let mut partitioner = PartitionWriter::new(
            config.output.clone(),
            config.partition_rows,
            config.partition_size,
        );

        let mut current_batches: Vec<RecordBatch> = Vec::new();

        for batch in batches {
            let batch = batch?;
            let batch_rows = batch.num_rows();
            let batch_bytes = estimate_batch_bytes(&batch);

            if partitioner.should_split(batch_rows, batch_bytes) && !current_batches.is_empty() {
                let out_path = partitioner.next_file();
                let stream: RecordBatchStream = Box::new(current_batches.into_iter().map(Ok));
                self.writer.write(&out_path, schema, stream)?;
                current_batches = Vec::new();
            }

            partitioner.add_batch(batch_rows, batch_bytes);
            current_batches.push(batch);
        }

        if !current_batches.is_empty() {
            let out_path = partitioner.next_file();
            let stream: RecordBatchStream = Box::new(current_batches.into_iter().map(Ok));
            self.writer.write(&out_path, schema, stream)?;
        }

        Ok(())
    }
}

fn estimate_batch_bytes(batch: &RecordBatch) -> u64 {
    let mut total = 0u64;
    for col in 0..batch.num_columns() {
        total += batch.column(col).get_buffer_memory_size() as u64;
    }
    total
}
