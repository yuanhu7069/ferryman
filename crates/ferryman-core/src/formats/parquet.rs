use std::path::Path;
use std::fs::File;
use std::sync::Arc;
use arrow::datatypes::Schema;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use crate::error::Result;
use crate::traits::{FormatReader, FormatWriter, RecordBatchStream};

pub struct ParquetReader;

impl FormatReader for ParquetReader {
    fn read(&self, path: &Path, _schema_override: Option<Schema>)
        -> Result<(Schema, RecordBatchStream)>
    {
        let file = File::open(path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let schema = builder.schema().as_ref().clone();
        let reader = builder.with_batch_size(1024).build()?;

        let mut batches = Vec::new();
        for batch in reader {
            batches.push(batch?);
        }

        let stream: RecordBatchStream = Box::new(batches.into_iter().map(Ok));
        Ok((schema, stream))
    }

    fn infer_schema(&self, path: &Path, _lines: usize) -> Result<Schema> {
        let file = File::open(path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        Ok(builder.schema().as_ref().clone())
    }

    fn format_name(&self) -> &'static str { "parquet" }
}

pub struct ParquetWriter {
    pub compression: parquet::basic::Compression,
}

impl Default for ParquetWriter {
    fn default() -> Self {
        ParquetWriter { compression: parquet::basic::Compression::SNAPPY }
    }
}

impl FormatWriter for ParquetWriter {
    fn write(&self, path: &Path, schema: &Schema, batches: RecordBatchStream) -> Result<()> {
        use parquet::file::properties::WriterProperties;

        let file = File::create(path)?;
        let props = WriterProperties::builder()
            .set_compression(self.compression)
            .build();
        let mut writer = ArrowWriter::try_new(file, Arc::new(schema.clone()), Some(props))?;

        for batch in batches {
            writer.write(&batch?)?;
        }
        writer.close()?;
        Ok(())
    }

    fn format_name(&self) -> &'static str { "parquet" }
}

pub fn compression_from_str(s: &str) -> Result<parquet::basic::Compression> {
    use parquet::basic::{Compression, GzipLevel, ZstdLevel, BrotliLevel};
    match s.to_lowercase().as_str() {
        "snappy" => Ok(Compression::SNAPPY),
        "gzip" | "gz" => Ok(Compression::GZIP(GzipLevel::default())),
        "zstd" | "zst" => Ok(Compression::ZSTD(ZstdLevel::default())),
        "lz4" => Ok(Compression::LZ4),
        "brotli" | "br" => Ok(Compression::BROTLI(BrotliLevel::default())),
        "none" | "uncompressed" => Ok(Compression::UNCOMPRESSED),
        other => Err(crate::error::FerrymanError::Config(
            format!("Unknown Parquet compression: '{}'. Supported: snappy, gzip, zstd, lz4, brotli, none", other)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Int64Array, StringArray};
    use arrow::datatypes::{DataType, Field};

    fn create_test_schema() -> Schema {
        Schema::new(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new("age", DataType::Int64, false),
        ])
    }

    fn create_test_batch() -> arrow::record_batch::RecordBatch {
        use arrow::record_batch::RecordBatch;
        let schema = Arc::new(create_test_schema());
        let name = StringArray::from(vec!["Alice", "Bob"]);
        let age = Int64Array::from(vec![30, 25]);
        RecordBatch::try_new(schema, vec![Arc::new(name), Arc::new(age)]).unwrap()
    }

    fn write_temp_parquet() -> (tempfile::NamedTempFile, Schema, std::path::PathBuf) {
        let schema = create_test_schema();
        let batch = create_test_batch();
        let output = tempfile::NamedTempFile::new().unwrap();
        let out_path = output.path().to_path_buf();

        let writer = ParquetWriter::default();
        let stream: RecordBatchStream = Box::new(vec![Ok(batch)].into_iter());
        writer.write(&out_path, &schema, stream).unwrap();

        (output, schema, out_path)
    }

    #[test]
    fn test_parquet_read_basic() {
        let (_f, _schema, path) = write_temp_parquet();

        let reader = ParquetReader;
        let (read_schema, mut batches) = reader.read(&path, None).unwrap();
        assert_eq!(read_schema.fields().len(), 2);
        let batch = batches.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 2);
    }

    #[test]
    fn test_parquet_roundtrip() {
        let (_f, _schema, path) = write_temp_parquet();

        let reader = ParquetReader;
        let (read_schema, mut batches) = reader.read(&path, None).unwrap();
        assert_eq!(read_schema.fields().len(), 2);
        let batch = batches.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 2);

        let name_col = batch.column(0).as_any().downcast_ref::<StringArray>().unwrap();
        let age_col = batch.column(1).as_any().downcast_ref::<Int64Array>().unwrap();
        assert_eq!(name_col.value(0), "Alice");
        assert_eq!(name_col.value(1), "Bob");
        assert_eq!(age_col.value(0), 30);
        assert_eq!(age_col.value(1), 25);
    }

    #[test]
    fn test_infer_schema() {
        let (_f, _schema, path) = write_temp_parquet();

        let reader = ParquetReader;
        let inferred = reader.infer_schema(&path, 100).unwrap();
        assert_eq!(inferred.fields().len(), 2);
        assert_eq!(inferred.field(0).name(), "name");
        assert_eq!(inferred.field(1).name(), "age");
    }

    #[test]
    fn test_compression_from_str() {
        use parquet::basic::Compression;
        assert_eq!(compression_from_str("snappy").unwrap(), Compression::SNAPPY);
        assert_eq!(compression_from_str("gzip").unwrap(), Compression::GZIP(parquet::basic::GzipLevel::default()));
        assert_eq!(compression_from_str("zstd").unwrap(), Compression::ZSTD(parquet::basic::ZstdLevel::default()));
        assert_eq!(compression_from_str("none").unwrap(), Compression::UNCOMPRESSED);
        assert!(compression_from_str("invalid").is_err());
    }
}
