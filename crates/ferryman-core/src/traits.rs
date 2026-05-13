use std::path::Path;
use arrow::datatypes::Schema;
use arrow::record_batch::RecordBatch;
use crate::error::Result;

pub type RecordBatchStream = Box<dyn Iterator<Item = Result<RecordBatch>>>;

pub trait FormatReader: Send + Sync {
    fn read(&self, path: &Path, schema_override: Option<Schema>)
        -> Result<(Schema, RecordBatchStream)>;

    fn infer_schema(&self, path: &Path, lines: usize)
        -> Result<Schema>;

    fn format_name(&self) -> &'static str;
}

pub trait FormatWriter: Send + Sync {
    fn write(&self, path: &Path, schema: &Schema, batches: RecordBatchStream)
        -> Result<()>;

    fn supports_streaming(&self) -> bool { true }

    fn format_name(&self) -> &'static str;
}
