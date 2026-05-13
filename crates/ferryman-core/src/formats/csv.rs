use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use arrow::datatypes::Schema;
use arrow_csv::ReaderBuilder;
use arrow_csv::reader::Format;
use crate::error::Result;
use crate::traits::{FormatReader, FormatWriter, RecordBatchStream};

pub struct CsvReader {
    pub delimiter: u8,
    pub has_header: bool,
    pub null_values: Vec<String>,
}

impl Default for CsvReader {
    fn default() -> Self {
        CsvReader {
            delimiter: b',',
            has_header: true,
            null_values: vec![String::new()],
        }
    }
}

impl FormatReader for CsvReader {
    fn read(&self, path: &Path, schema_override: Option<Schema>)
        -> Result<(Schema, RecordBatchStream)>
    {
        let schema = if let Some(s) = schema_override {
            s
        } else {
            self.infer_schema(path, 100)?
        };

        let file = open_maybe_compressed(path)?;
        let mut csv_reader = ReaderBuilder::new(Arc::new(schema))
            .with_delimiter(self.delimiter)
            .with_header(self.has_header)
            .build(file)?;

        let schema = csv_reader.schema().as_ref().clone();

        let mut batches = Vec::new();
        while let Some(batch_result) = csv_reader.next() {
            let batch = batch_result?;
            batches.push(batch);
        }

        let stream: RecordBatchStream = Box::new(batches.into_iter().map(Ok));
        Ok((schema, stream))
    }

    fn infer_schema(&self, path: &Path, lines: usize) -> Result<Schema> {
        let file = open_maybe_compressed(path)?;
        let format = Format::default()
            .with_delimiter(self.delimiter)
            .with_header(self.has_header);
        let (schema, _) = format.infer_schema(file, Some(lines))?;
        Ok(schema)
    }

    fn format_name(&self) -> &'static str { "csv" }
}

pub struct CsvWriter {
    pub delimiter: u8,
    pub null_repr: String,
}

impl Default for CsvWriter {
    fn default() -> Self {
        CsvWriter {
            delimiter: b',',
            null_repr: String::new(),
        }
    }
}

impl FormatWriter for CsvWriter {
    fn write(&self, path: &Path, _schema: &Schema, batches: RecordBatchStream) -> Result<()> {
        use arrow_csv::WriterBuilder;
        let file = File::create(path)?;
        let mut writer = WriterBuilder::new()
            .with_delimiter(self.delimiter)
            .build(file);

        for batch in batches {
            let batch = batch?;
            writer.write(&batch)?;
        }
        drop(writer);
        Ok(())
    }

    fn format_name(&self) -> &'static str { "csv" }
}

fn open_maybe_compressed(path: &Path) -> Result<BufReader<Box<dyn std::io::Read + Send>>> {
    let file = File::open(path)?;
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let reader: Box<dyn std::io::Read + Send> = match ext.as_str() {
        "gz" | "gzip" => Box::new(flate2::read::GzDecoder::new(file)),
        "bz2" | "bzip2" => Box::new(bzip2::read::BzDecoder::new(file)),
        "xz" => Box::new(xz2::read::XzDecoder::new(file)),
        "zst" | "zstd" => Box::new(zstd::stream::read::Decoder::new(file)?),
        _ => Box::new(file),
    };
    Ok(BufReader::new(reader))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_temp_csv(content: &str) -> (tempfile::NamedTempFile, std::path::PathBuf) {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        let path = file.path().to_path_buf();
        (file, path)
    }

    #[test]
    fn test_csv_read_basic() {
        let (_f, path) = create_temp_csv("name,age\nAlice,30\nBob,25\n");
        let reader = CsvReader::default();
        let (schema, mut batches) = reader.read(&path, None).unwrap();
        assert_eq!(schema.fields().len(), 2);
        let batch = batches.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 2);
    }

    #[test]
    fn test_csv_roundtrip() {
        let (_f, input_path) = create_temp_csv("name,age\nAlice,30\nBob,25\n");
        let output = tempfile::NamedTempFile::new().unwrap();
        let out_path = output.path().to_path_buf();

        let reader = CsvReader::default();
        let (schema, batches) = reader.read(&input_path, None).unwrap();
        let writer = CsvWriter::default();
        writer.write(&out_path, &schema, batches).unwrap();

        let reader2 = CsvReader::default();
        let (schema2, mut batches2) = reader2.read(&out_path, None).unwrap();
        assert_eq!(schema2.fields().len(), 2);
        let batch = batches2.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 2);
    }
}
