use std::path::Path;
use std::fs::File;
use arrow::datatypes::Schema;
use arrow::ipc::{reader::FileReader, writer::FileWriter};
use crate::error::Result;
use crate::traits::{FormatReader, FormatWriter, RecordBatchStream};

pub struct ArrowIpcReader;

impl FormatReader for ArrowIpcReader {
    fn read(&self, path: &Path, _schema_override: Option<Schema>)
        -> Result<(Schema, RecordBatchStream)>
    {
        let file = File::open(path)?;
        let reader = FileReader::try_new(file, None)?;
        let schema = reader.schema().as_ref().clone();

        let mut batches = Vec::new();
        for batch in reader {
            batches.push(batch?);
        }

        let stream: RecordBatchStream = Box::new(batches.into_iter().map(Ok));
        Ok((schema, stream))
    }

    fn infer_schema(&self, path: &Path, _lines: usize) -> Result<Schema> {
        let file = File::open(path)?;
        let reader = FileReader::try_new(file, None)?;
        Ok(reader.schema().as_ref().clone())
    }

    fn format_name(&self) -> &'static str { "arrow" }
}

pub struct ArrowIpcWriter;

impl FormatWriter for ArrowIpcWriter {
    fn write(&self, path: &Path, schema: &Schema, batches: RecordBatchStream) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = FileWriter::try_new(file, schema)?;

        for batch in batches {
            writer.write(&batch?)?;
        }
        writer.finish()?;
        Ok(())
    }

    fn format_name(&self) -> &'static str { "arrow" }
}
