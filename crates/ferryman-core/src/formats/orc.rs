use std::path::Path;
use std::fs::File;
use arrow::datatypes::Schema;
use arrow::record_batch::RecordBatchReader;
use orc_rust::ArrowReaderBuilder;
use crate::error::{FerrymanError, Result};
use crate::traits::{FormatReader, FormatWriter, RecordBatchStream};

pub struct OrcReader;

impl FormatReader for OrcReader {
    fn read(&self, path: &Path, _schema_override: Option<Schema>)
        -> Result<(Schema, RecordBatchStream)>
    {
        let file = File::open(path)?;
        let reader = ArrowReaderBuilder::try_new(file)
            .map_err(|e| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
            .build();

        let schema = reader.schema().as_ref().clone();

        let mut batches = Vec::new();
        for batch in reader {
            batches.push(batch.map_err(|e| FerrymanError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            ))?);
        }

        let stream: RecordBatchStream = Box::new(batches.into_iter().map(Ok));
        Ok((schema, stream))
    }

    fn infer_schema(&self, path: &Path, _lines: usize) -> Result<Schema> {
        let file = File::open(path)?;
        let reader = ArrowReaderBuilder::try_new(file)
            .map_err(|e| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
            .build();
        Ok(reader.schema().as_ref().clone())
    }

    fn format_name(&self) -> &'static str { "orc" }
}

pub struct OrcWriter;

impl FormatWriter for OrcWriter {
    fn write(&self, _path: &Path, _schema: &Schema, _batches: RecordBatchStream) -> Result<()> {
        Err(FerrymanError::OrcWriteNotSupported {
            src: "unknown".into(),
            input: "unknown".into(),
        })
    }

    fn format_name(&self) -> &'static str { "orc" }
}
