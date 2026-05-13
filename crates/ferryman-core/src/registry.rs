use std::path::Path;
use std::collections::HashMap;
use crate::traits::{FormatReader, FormatWriter};
use crate::error::{FerrymanError, Result};

const EXTENSION_MAP: &[(&str, &str)] = &[
    ("csv", "csv"),
    ("tsv", "csv"),
    ("json", "json"),
    ("jsonl", "json"),
    ("xlsx", "excel"),
    ("xls", "excel"),
    ("parquet", "parquet"),
    ("pq", "parquet"),
    ("arrow", "arrow"),
    ("feather", "arrow"),
    ("ipc", "arrow"),
    ("orc", "orc"),
    ("avro", "avro"),
];

pub struct FormatRegistry {
    readers: HashMap<String, Box<dyn Fn() -> Box<dyn FormatReader>>>,
    writers: HashMap<String, Box<dyn Fn() -> Box<dyn FormatWriter>>>,
}

impl FormatRegistry {
    pub fn new() -> Self {
        FormatRegistry {
            readers: HashMap::new(),
            writers: HashMap::new(),
        }
    }

    pub fn register_reader<F>(&mut self, name: &str, factory: F)
    where
        F: Fn() -> Box<dyn FormatReader> + 'static,
    {
        self.readers.insert(name.to_string(), Box::new(factory));
    }

    pub fn register_writer<F>(&mut self, name: &str, factory: F)
    where
        F: Fn() -> Box<dyn FormatWriter> + 'static,
    {
        self.writers.insert(name.to_string(), Box::new(factory));
    }

    pub fn get_reader(&self, name: &str) -> Result<Box<dyn FormatReader>> {
        self.readers.get(name)
            .map(|f| f())
            .ok_or_else(|| FerrymanError::UnsupportedFormat(name.to_string()))
    }

    pub fn get_writer(&self, name: &str) -> Result<Box<dyn FormatWriter>> {
        self.writers.get(name)
            .map(|f| f())
            .ok_or_else(|| FerrymanError::UnsupportedFormat(name.to_string()))
    }

    pub fn list_formats(&self) -> Vec<String> {
        let mut names: Vec<String> = self.readers.keys().cloned().collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn supports_read(&self, name: &str) -> bool {
        self.readers.contains_key(name)
    }

    pub fn supports_write(&self, name: &str) -> bool {
        self.writers.contains_key(name)
    }
}

pub fn detect_format(path: &Path) -> Result<&'static str> {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let base_ext = match ext.as_str() {
        "gz" | "gzip" | "bz2" | "bzip2" | "xz" | "zst" | "zstd" => {
            let stem = path.with_extension("");
            stem.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase()
        }
        _ => ext,
    };

    for (ext, format) in EXTENSION_MAP {
        if *ext == base_ext {
            return Ok(format);
        }
    }
    Err(FerrymanError::UnsupportedFormat(format!(
        "Cannot detect format from extension '.{}'. Use --from/--to to specify.", base_ext
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_csv() {
        assert_eq!(detect_format(&PathBuf::from("data.csv")).unwrap(), "csv");
    }

    #[test]
    fn test_detect_parquet() {
        assert_eq!(detect_format(&PathBuf::from("data.parquet")).unwrap(), "parquet");
    }

    #[test]
    fn test_detect_jsonl() {
        assert_eq!(detect_format(&PathBuf::from("data.jsonl")).unwrap(), "json");
    }

    #[test]
    fn test_detect_compressed_csv() {
        assert_eq!(detect_format(&PathBuf::from("data.csv.gz")).unwrap(), "csv");
    }

    #[test]
    fn test_detect_unknown() {
        assert!(detect_format(&PathBuf::from("data.xyz")).is_err());
    }

    #[test]
    fn test_registry_reader_not_found() {
        let reg = FormatRegistry::new();
        assert!(reg.get_reader("nonexistent").is_err());
    }

    #[test]
    fn test_registry_writer_not_found() {
        let reg = FormatRegistry::new();
        assert!(reg.get_writer("nonexistent").is_err());
    }
}
