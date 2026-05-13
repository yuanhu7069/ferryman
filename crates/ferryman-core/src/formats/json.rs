use std::path::Path;
use std::fs;
use std::io::{BufReader, Cursor};
use std::sync::Arc;
use arrow::datatypes::Schema;
use arrow::json::ReaderBuilder;
use crate::error::Result;
use crate::traits::{FormatReader, FormatWriter, RecordBatchStream};

pub struct JsonReader;

impl JsonReader {
    fn read_raw(&self, path: &Path) -> Result<(Vec<u8>, bool)> {
        let raw = fs::read(path)?;
        let is_array = raw.iter().copied()
            .find(|b| !b.is_ascii_whitespace())
            .map_or(false, |b| b == b'[');
        Ok((raw, is_array))
    }

    fn array_to_jsonl(&self, raw: &[u8]) -> Result<Vec<u8>> {
        let values: Vec<serde_json::Value> = serde_json::from_slice(raw)?;
        let mut buf = Vec::with_capacity(raw.len());
        for v in values {
            serde_json::to_writer(&mut buf, &v)?;
            buf.push(b'\n');
        }
        Ok(buf)
    }

    fn infer_schema_from_bytes(&self, raw: &[u8], is_array: bool, max_records: usize) -> Result<Schema> {
        if is_array {
            let values: Vec<serde_json::Value> = serde_json::from_slice(raw)?;
            let schema = arrow::json::reader::infer_json_schema_from_iterator(
                values.into_iter().take(max_records).map(Ok),
            )?;
            Ok(schema)
        } else {
            let reader = BufReader::new(Cursor::new(raw));
            let (schema, _) = arrow::json::reader::infer_json_schema(reader, Some(max_records))?;
            Ok(schema)
        }
    }

    fn jsonl_from_bytes(&self, raw: Vec<u8>, is_array: bool) -> Result<Vec<u8>> {
        if is_array {
            self.array_to_jsonl(&raw)
        } else {
            Ok(raw)
        }
    }
}

impl FormatReader for JsonReader {
    fn read(&self, path: &Path, schema_override: Option<Schema>)
        -> Result<(Schema, RecordBatchStream)>
    {
        let (raw, is_array) = self.read_raw(path)?;

        let schema = if let Some(s) = schema_override {
            s
        } else {
            self.infer_schema_from_bytes(&raw, is_array, 100)?
        };

        let jsonl = self.jsonl_from_bytes(raw, is_array)?;
        let cursor = Cursor::new(jsonl);
        let json_reader = ReaderBuilder::new(Arc::new(schema.clone())).build(cursor)?;

        let mut batches = Vec::new();
        for batch in json_reader {
            batches.push(batch?);
        }

        let stream: RecordBatchStream = Box::new(batches.into_iter().map(Ok));
        Ok((schema, stream))
    }

    fn infer_schema(&self, path: &Path, _lines: usize) -> Result<Schema> {
        let (raw, is_array) = self.read_raw(path)?;
        self.infer_schema_from_bytes(&raw, is_array, _lines)
    }

    fn format_name(&self) -> &'static str { "json" }
}

pub struct JsonWriter;

impl FormatWriter for JsonWriter {
    fn write(&self, path: &Path, _schema: &Schema, batches: RecordBatchStream) -> Result<()> {
        use arrow::json::LineDelimitedWriter;
        let file = fs::File::create(path)?;
        let mut writer = LineDelimitedWriter::new(file);

        for batch in batches {
            writer.write(&batch?)?;
        }
        writer.finish()?;
        Ok(())
    }

    fn format_name(&self) -> &'static str { "json" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_temp_json(content: &str) -> (tempfile::NamedTempFile, std::path::PathBuf) {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        let path = file.path().to_path_buf();
        (file, path)
    }

    #[test]
    fn test_json_read_array() {
        let (_f, path) = create_temp_json(
            r#"[{"name":"Alice","age":30},{"name":"Bob","age":25}]"#
        );
        let reader = JsonReader;
        let (schema, mut batches) = reader.read(&path, None).unwrap();
        assert_eq!(schema.fields().len(), 2);
        let batch = batches.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 2);
    }

    #[test]
    fn test_json_read_jsonl() {
        let (_f, path) = create_temp_json(
            "{\"name\":\"Alice\",\"age\":30}\n{\"name\":\"Bob\",\"age\":25}\n"
        );
        let reader = JsonReader;
        let (schema, mut batches) = reader.read(&path, None).unwrap();
        assert_eq!(schema.fields().len(), 2);
        let batch = batches.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 2);
    }

    #[test]
    fn test_json_roundtrip() {
        let (_f, input_path) = create_temp_json(
            r#"[{"name":"Alice","age":30},{"name":"Bob","age":25}]"#
        );
        let output = tempfile::NamedTempFile::new().unwrap();
        let out_path = output.path().to_path_buf();

        let reader = JsonReader;
        let (schema, batches) = reader.read(&input_path, None).unwrap();
        let writer = JsonWriter;
        writer.write(&out_path, &schema, batches).unwrap();

        let reader2 = JsonReader;
        let (schema2, mut batches2) = reader2.read(&out_path, None).unwrap();
        assert_eq!(schema2.fields().len(), 2);
        let batch = batches2.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 2);
    }
}
