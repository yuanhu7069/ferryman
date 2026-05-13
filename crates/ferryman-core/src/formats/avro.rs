use std::collections::BTreeMap;
use std::path::Path;
use std::fs::File;
use std::sync::Arc;
use arrow::array::{ArrayRef, Int64Builder, Float64Builder, StringBuilder, BooleanBuilder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use apache_avro::{Reader as AvroReader, Writer as AvroWriter, Schema as AvroSchema};
use apache_avro::schema::{RecordField, RecordFieldOrder, RecordSchema};
use apache_avro::types::Value;
use crate::error::{FerrymanError, Result};
use crate::traits::{FormatReader, FormatWriter, RecordBatchStream};

pub struct AvroFormatReader;

impl FormatReader for AvroFormatReader {
    fn read(&self, path: &Path, _schema_override: Option<Schema>)
        -> Result<(Schema, RecordBatchStream)>
    {
        let file = File::open(path)?;
        let reader = AvroReader::new(file)
            .map_err(|e| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

        let avro_schema = reader.writer_schema().clone();
        let arrow_schema = avro_schema_to_arrow(&avro_schema)?;
        let fields = arrow_schema.fields().clone();
        let n_cols = fields.len();

        let mut column_builders: Vec<Box<dyn ColumnBuilder>> = fields.iter()
            .map(|f| make_builder(f.data_type()))
            .collect();

        for record in reader {
            let record = record.map_err(|e| FerrymanError::ConversionError {
                row: 0, message: e.to_string(),
            })?;
            if let Value::Record(field_values) = record {
                for (i, (_name, value)) in field_values.iter().enumerate() {
                    if i < n_cols {
                        append_value(&mut *column_builders[i], value);
                    }
                }
            }
        }

        let arrays: Vec<ArrayRef> = column_builders.into_iter()
            .map(|mut b| b.finish())
            .collect();

        let batch = RecordBatch::try_new(Arc::new(arrow_schema.clone()), arrays)?;
        let stream: RecordBatchStream = Box::new(std::iter::once(Ok(batch)));
        Ok((arrow_schema, stream))
    }

    fn infer_schema(&self, path: &Path, _lines: usize) -> Result<Schema> {
        let file = File::open(path)?;
        let reader = AvroReader::new(file)
            .map_err(|e| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        avro_schema_to_arrow(reader.writer_schema())
    }

    fn format_name(&self) -> &'static str { "avro" }
}

trait ColumnBuilder {
    fn append_str(&mut self, s: &str);
    fn append_null(&mut self);
    fn finish(&mut self) -> ArrayRef;
}

fn make_builder(dt: &DataType) -> Box<dyn ColumnBuilder> {
    match dt {
        DataType::Int64 => Box::new(Int64ColBuilder::new()),
        DataType::Float64 => Box::new(Float64ColBuilder::new()),
        DataType::Boolean => Box::new(BoolColBuilder::new()),
        _ => Box::new(StringColBuilder::new()),
    }
}

struct Int64ColBuilder { builder: Int64Builder }
impl ColumnBuilder for Int64ColBuilder {
    fn append_str(&mut self, s: &str) { self.builder.append_value(s.parse().unwrap_or(0)); }
    fn append_null(&mut self) { self.builder.append_null(); }
    fn finish(&mut self) -> ArrayRef { Arc::new(self.builder.finish()) }
}
impl Int64ColBuilder { fn new() -> Self { Self { builder: Int64Builder::new() } } }

struct Float64ColBuilder { builder: Float64Builder }
impl ColumnBuilder for Float64ColBuilder {
    fn append_str(&mut self, s: &str) { self.builder.append_value(s.parse().unwrap_or(0.0)); }
    fn append_null(&mut self) { self.builder.append_null(); }
    fn finish(&mut self) -> ArrayRef { Arc::new(self.builder.finish()) }
}
impl Float64ColBuilder { fn new() -> Self { Self { builder: Float64Builder::new() } } }

struct BoolColBuilder { builder: BooleanBuilder }
impl ColumnBuilder for BoolColBuilder {
    fn append_str(&mut self, s: &str) { self.builder.append_value(s.eq_ignore_ascii_case("true")); }
    fn append_null(&mut self) { self.builder.append_null(); }
    fn finish(&mut self) -> ArrayRef { Arc::new(self.builder.finish()) }
}
impl BoolColBuilder { fn new() -> Self { Self { builder: BooleanBuilder::new() } } }

struct StringColBuilder { builder: StringBuilder }
impl ColumnBuilder for StringColBuilder {
    fn append_str(&mut self, s: &str) { self.builder.append_value(s); }
    fn append_null(&mut self) { self.builder.append_null(); }
    fn finish(&mut self) -> ArrayRef { Arc::new(self.builder.finish()) }
}
impl StringColBuilder { fn new() -> Self { Self { builder: StringBuilder::new() } } }

fn append_value(builder: &mut dyn ColumnBuilder, value: &Value) {
    match value {
        Value::Null => builder.append_null(),
        Value::Boolean(b) => builder.append_str(if *b { "true" } else { "false" }),
        Value::Int(i) => builder.append_str(&i.to_string()),
        Value::Long(i) => builder.append_str(&i.to_string()),
        Value::Float(f) => builder.append_str(&f.to_string()),
        Value::Double(f) => builder.append_str(&f.to_string()),
        Value::String(s) | Value::Enum(_, s) => builder.append_str(s),
        Value::Bytes(_) | Value::Fixed(_, _) => builder.append_str("<binary>"),
        Value::Array(arr) => builder.append_str(&format!("{:?}", arr)),
        Value::Map(m) => builder.append_str(&format!("{:?}", m)),
        Value::Record(fields) => builder.append_str(&format!("{:?}", fields)),
        Value::Union(_, boxed) => append_value(builder, boxed),
        _ => builder.append_null(),
    }
}

fn avro_schema_to_arrow(schema: &AvroSchema) -> Result<Schema> {
    match schema {
        AvroSchema::Record(record_schema) => {
            let arrow_fields: Vec<Field> = record_schema.fields.iter().map(|f| {
                let dt = avro_field_to_arrow_type(&f.schema);
                Field::new(&f.name, dt, true)
            }).collect();
            Ok(Schema::new(arrow_fields))
        }
        _ => Err(FerrymanError::Config("Avro schema must be a record type".into())),
    }
}

fn avro_field_to_arrow_type(schema: &AvroSchema) -> DataType {
    match schema {
        AvroSchema::Int | AvroSchema::Long => DataType::Int64,
        AvroSchema::Float | AvroSchema::Double => DataType::Float64,
        AvroSchema::Boolean => DataType::Boolean,
        AvroSchema::String | AvroSchema::Enum { .. } => DataType::Utf8,
        AvroSchema::Bytes | AvroSchema::Fixed { .. } => DataType::Binary,
        AvroSchema::Null => DataType::Utf8,
        AvroSchema::Union(union_schema) => {
            union_schema.variants().iter()
                .find(|v| !matches!(v, AvroSchema::Null))
                .map(|v| avro_field_to_arrow_type(v))
                .unwrap_or(DataType::Utf8)
        }
        _ => DataType::Utf8,
    }
}

pub struct AvroFormatWriter;

impl FormatWriter for AvroFormatWriter {
    fn write(&self, path: &Path, schema: &Schema, batches: RecordBatchStream) -> Result<()> {
        let avro_schema = arrow_schema_to_avro(schema)?;
        let file = File::create(path)?;
        let mut writer = AvroWriter::new(&avro_schema, file);

        for batch in batches {
            let batch = batch?;
            for row in 0..batch.num_rows() {
                let mut record = Vec::new();
                for col in 0..batch.num_columns() {
                    let name = schema.field(col).name().clone();
                    let array = batch.column(col);
                    let value = if array.is_null(row) {
                        Value::Null
                    } else {
                        match array.data_type() {
                            DataType::Int64 => {
                                let arr = array.as_any().downcast_ref::<arrow::array::Int64Array>().unwrap();
                                Value::Long(arr.value(row))
                            }
                            DataType::Float64 => {
                                let arr = array.as_any().downcast_ref::<arrow::array::Float64Array>().unwrap();
                                Value::Double(arr.value(row))
                            }
                            DataType::Boolean => {
                                let arr = array.as_any().downcast_ref::<arrow::array::BooleanArray>().unwrap();
                                Value::Boolean(arr.value(row))
                            }
                            _ => Value::String(format!("{:?}", array)),
                        }
                    };
                    record.push((name, value));
                }
                writer.append(Value::Record(record))
                    .map_err(|e| FerrymanError::ConversionError { row, message: e.to_string() })?;
            }
        }
        writer.flush()
            .map_err(|e| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        Ok(())
    }

    fn format_name(&self) -> &'static str { "avro" }
}

fn arrow_schema_to_avro(schema: &Schema) -> Result<AvroSchema> {
    let fields: Vec<RecordField> = schema.fields().iter().map(|f| {
        let avro_dt = match f.data_type() {
            DataType::Int64 => AvroSchema::Long,
            DataType::Float64 => AvroSchema::Double,
            DataType::Boolean => AvroSchema::Boolean,
            _ => AvroSchema::String,
        };
        RecordField {
            name: f.name().clone(),
            doc: None,
            aliases: None,
            default: None,
            schema: avro_dt,
            order: RecordFieldOrder::Ignore,
            position: 0,
            custom_attributes: BTreeMap::new(),
        }
    }).collect();

    Ok(AvroSchema::Record(RecordSchema {
        name: apache_avro::schema::Name::new("ferryman_record")
            .map_err(|e| FerrymanError::Config(e.to_string()))?,
        aliases: None,
        doc: None,
        fields,
        lookup: Default::default(),
        attributes: BTreeMap::new(),
    }))
}
