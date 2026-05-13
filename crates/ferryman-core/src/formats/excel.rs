use std::path::Path;
use std::sync::Arc;
use arrow::array::{Array, ArrayRef, StringBuilder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use arrow::util::display::array_value_to_string;
use calamine::{open_workbook, Reader, Xlsx, Data};
use rust_xlsxwriter::Workbook;
use crate::error::{FerrymanError, Result};
use crate::traits::{FormatReader, FormatWriter, RecordBatchStream};

pub struct ExcelReader {
    pub sheet_name: Option<String>,
}

impl Default for ExcelReader {
    fn default() -> Self {
        ExcelReader { sheet_name: None }
    }
}

impl FormatReader for ExcelReader {
    fn read(&self, path: &Path, _schema_override: Option<Schema>)
        -> Result<(Schema, RecordBatchStream)>
    {
        let mut workbook: Xlsx<_> = open_workbook(path)
            .map_err(|e: calamine::XlsxError| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

        let sheet_name = if let Some(ref name) = self.sheet_name {
            name.clone()
        } else {
            workbook.sheet_names().first()
                .ok_or_else(|| FerrymanError::Config("Excel file has no sheets".into()))?
                .clone()
        };

        let range = workbook.worksheet_range(&sheet_name)
            .map_err(|e| FerrymanError::Config(format!("Failed to read sheet '{}': {}", sheet_name, e)))?;

        let mut rows_iter = range.rows();
        let headers: Vec<String> = match rows_iter.next() {
            Some(row) => row.iter().map(|cell: &Data| cell.to_string()).collect(),
            None => return Err(FerrymanError::Config("Empty sheet".into())),
        };

        let n_cols = headers.len();
        let schema = Schema::new(
            headers.iter().map(|h| Field::new(h, DataType::Utf8, true)).collect::<Vec<_>>()
        );

        let mut columns: Vec<StringBuilder> = (0..n_cols).map(|_| StringBuilder::new()).collect();
        for row in rows_iter {
            for (i, cell) in row.iter().enumerate() {
                if i < n_cols {
                    let val = match cell {
                        Data::Empty => "",
                        Data::String(s) => s,
                        Data::Float(f) => &f.to_string(),
                        Data::Int(i) => &i.to_string(),
                        Data::Bool(b) => if *b { "true" } else { "false" },
                        Data::Error(_) => "",
                        _ => &cell.to_string(),
                    };
                    columns[i].append_value(val);
                }
            }
            for i in row.len()..n_cols {
                columns[i].append_null();
            }
        }

        let arrays: Vec<ArrayRef> = columns.into_iter()
            .map(|mut b| Arc::new(b.finish()) as ArrayRef)
            .collect();

        let batch = RecordBatch::try_new(Arc::new(schema.clone()), arrays)?;
        let stream: RecordBatchStream = Box::new(std::iter::once(Ok(batch)));
        Ok((schema, stream))
    }

    fn infer_schema(&self, path: &Path, _lines: usize) -> Result<Schema> {
        let mut workbook: Xlsx<_> = open_workbook(path)
            .map_err(|e: calamine::XlsxError| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        let sheet_name = if let Some(ref name) = self.sheet_name {
            name.clone()
        } else {
            workbook.sheet_names().first()
                .ok_or_else(|| FerrymanError::Config("Excel file has no sheets".into()))?
                .clone()
        };
        let range = workbook.worksheet_range(&sheet_name)
            .map_err(|e| FerrymanError::Config(format!("Failed to read sheet '{}': {}", sheet_name, e)))?;
        let headers: Vec<String> = range.rows().next()
            .map(|row: &[Data]| row.iter().map(|c: &Data| c.to_string()).collect())
            .unwrap_or_default();
        Ok(Schema::new(
            headers.iter().map(|h| Field::new(h, DataType::Utf8, true)).collect::<Vec<_>>()
        ))
    }

    fn format_name(&self) -> &'static str { "excel" }
}

pub struct ExcelWriter;

impl FormatWriter for ExcelWriter {
    fn write(&self, path: &Path, schema: &Schema, batches: RecordBatchStream) -> Result<()> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        for (col, field) in schema.fields().iter().enumerate() {
            worksheet.write_string(0, col as u16, field.name())
                .map_err(|e| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        }

        let mut row_offset: u32 = 1;
        for batch in batches {
            let batch = batch?;
            for row in 0..batch.num_rows() {
                for col in 0..batch.num_columns() {
                    let col_array = batch.column(col);
                    if col_array.is_null(row) { continue; }
                    let val = array_value_to_string(col_array, row)
                        .map_err(|e| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
                    worksheet.write_string(row_offset, col as u16, &val)
                        .map_err(|e| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
                }
                row_offset += 1;
            }
        }

        workbook.save(path)
            .map_err(|e| FerrymanError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        Ok(())
    }

    fn supports_streaming(&self) -> bool { false }

    fn format_name(&self) -> &'static str { "excel" }
}
