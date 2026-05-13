use std::path::Path;
use arrow::datatypes::{Schema, DataType, Field};
use serde::{Deserialize, Serialize};
use crate::error::{FerrymanError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSchema {
    pub columns: Vec<UserColumn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub data_type: String,
}

impl UserSchema {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let schema: UserSchema = serde_json::from_str(&content)?;
        Ok(schema)
    }

    pub fn to_arrow_schema(&self) -> Result<Schema> {
        let fields: Result<Vec<Field>> = self.columns.iter().map(|col| {
            let dt = string_to_data_type(&col.data_type)?;
            Ok(Field::new(&col.name, dt, true))
        }).collect();
        Ok(Schema::new(fields?))
    }
}

fn string_to_data_type(s: &str) -> Result<DataType> {
    match s.to_lowercase().as_str() {
        "int8" => Ok(DataType::Int8),
        "int16" => Ok(DataType::Int16),
        "int32" => Ok(DataType::Int32),
        "int64" => Ok(DataType::Int64),
        "uint8" => Ok(DataType::UInt8),
        "uint16" => Ok(DataType::UInt16),
        "uint32" => Ok(DataType::UInt32),
        "uint64" => Ok(DataType::UInt64),
        "float32" | "float" => Ok(DataType::Float32),
        "float64" | "double" => Ok(DataType::Float64),
        "utf8" | "string" | "str" => Ok(DataType::Utf8),
        "largeutf8" | "largestring" => Ok(DataType::LargeUtf8),
        "bool" | "boolean" => Ok(DataType::Boolean),
        "date32" | "date" => Ok(DataType::Date32),
        "date64" => Ok(DataType::Date64),
        "timestamp_ms" | "timestamp" => Ok(DataType::Timestamp(
            arrow::datatypes::TimeUnit::Millisecond, None)),
        "timestamp_us" => Ok(DataType::Timestamp(
            arrow::datatypes::TimeUnit::Microsecond, None)),
        "timestamp_ns" => Ok(DataType::Timestamp(
            arrow::datatypes::TimeUnit::Nanosecond, None)),
        "binary" => Ok(DataType::Binary),
        "largebinary" => Ok(DataType::LargeBinary),
        other => Err(FerrymanError::Config(
            format!("Unknown type: '{}'. Supported: int8-int64, uint8-uint64, float32, float64, utf8, bool, date32, date64, timestamp_ms, timestamp_us, timestamp_ns, binary", other)
        )),
    }
}

pub struct SchemaAdapter;

impl SchemaAdapter {
    pub fn adapt(source: &Schema, _target_has_schema: bool) -> Schema {
        source.clone()
    }

    pub fn has_typed_schema(schema: &Schema) -> bool {
        schema.fields().iter().any(|f| !matches!(f.data_type(), DataType::Utf8 | DataType::LargeUtf8))
    }
}

pub fn arrow_value_to_json_string(array: &dyn arrow::array::Array, row: usize) -> Result<String> {
    use arrow::array::*;
    use std::fmt::Write;

    if array.is_null(row) {
        return Ok("null".to_string());
    }

    let mut json = String::new();
    match array.data_type() {
        DataType::Struct(fields) => {
            let struct_arr = array.as_any().downcast_ref::<StructArray>()
                .ok_or_else(|| FerrymanError::ConversionError {
                    row, message: "failed to cast to StructArray".into()
                })?;
            json.push('{');
            for (i, field) in fields.iter().enumerate() {
                if i > 0 { json.push_str(", "); }
                write!(&mut json, "\"{}\": ", field.name()).unwrap();
                let val = arrow_value_to_json_string(struct_arr.column(i), row)?;
                json.push_str(&val);
            }
            json.push('}');
        }
        DataType::List(_) | DataType::LargeList(_) => {
            json.push_str(&format!("{:?}", array));
        }
        _ => {
            json.push_str(&format!("{:?}", array));
        }
    }
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_user_schema_json() {
        let json = r#"{"columns": [{"name": "id", "type": "Int64"}, {"name": "name", "type": "Utf8"}]}"#;
        let schema: UserSchema = serde_json::from_str(json).unwrap();
        assert_eq!(schema.columns.len(), 2);
        assert_eq!(schema.columns[0].name, "id");
        assert_eq!(schema.columns[0].data_type, "Int64");
    }

    #[test]
    fn test_string_to_data_type_mappings() {
        assert!(matches!(string_to_data_type("Int64").unwrap(), DataType::Int64));
        assert!(matches!(string_to_data_type("utf8").unwrap(), DataType::Utf8));
        assert!(matches!(string_to_data_type("Float64").unwrap(), DataType::Float64));
        assert!(matches!(string_to_data_type("bool").unwrap(), DataType::Boolean));
    }

    #[test]
    fn test_unknown_type_returns_error() {
        assert!(string_to_data_type("mythical").is_err());
    }

    #[test]
    fn test_has_typed_schema() {
        let schema = Schema::new(vec![
            Field::new("name", DataType::Utf8, true),
            Field::new("age", DataType::Int64, true),
        ]);
        assert!(SchemaAdapter::has_typed_schema(&schema));

        let schema = Schema::new(vec![
            Field::new("a", DataType::Utf8, true),
            Field::new("b", DataType::Utf8, true),
        ]);
        assert!(!SchemaAdapter::has_typed_schema(&schema));
    }
}
