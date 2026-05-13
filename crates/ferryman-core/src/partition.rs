use std::path::{Path, PathBuf};

pub struct PartitionWriter {
    output_base: PathBuf,
    max_rows: Option<usize>,
    max_bytes: Option<u64>,
    current_rows: usize,
    current_bytes: u64,
    partition_index: usize,
}

impl PartitionWriter {
    pub fn new(output_base: PathBuf, max_rows: Option<usize>, max_bytes: Option<u64>) -> Self {
        PartitionWriter {
            output_base,
            max_rows,
            max_bytes,
            current_rows: 0,
            current_bytes: 0,
            partition_index: 0,
        }
    }

    pub fn should_split(&self, next_rows: usize, next_bytes: u64) -> bool {
        if let Some(max) = self.max_rows {
            if self.current_rows + next_rows > max {
                return true;
            }
        }
        if let Some(max) = self.max_bytes {
            if self.current_bytes + next_bytes > max {
                return true;
            }
        }
        false
    }

    pub fn next_file(&mut self) -> PathBuf {
        let path = make_partition_path(&self.output_base, self.partition_index);
        self.partition_index += 1;
        path
    }

    pub fn add_batch(&mut self, rows: usize, bytes: u64) {
        self.current_rows += rows;
        self.current_bytes += bytes;
    }
}

fn make_partition_path(base: &Path, index: usize) -> PathBuf {
    let stem = base.file_stem().unwrap_or_default().to_string_lossy();
    let ext = base.extension().map(|e| e.to_string_lossy().to_string());
    let parent = base.parent().unwrap_or_else(|| Path::new("."));

    let new_stem = format!("{}.part{:04}", stem, index);
    match ext {
        Some(e) => parent.join(Path::new(&new_stem).with_extension(e)),
        None => parent.join(new_stem),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_paths() {
        let base = Path::new("/out/data.parquet");
        let p0 = make_partition_path(base, 0);
        let p1 = make_partition_path(base, 5);
        assert_eq!(p0, PathBuf::from("/out/data.part0000.parquet"));
        assert_eq!(p1, PathBuf::from("/out/data.part0005.parquet"));
    }

    #[test]
    fn test_partition_writer_should_split_by_rows() {
        let mut pw = PartitionWriter::new(
            PathBuf::from("out.csv"),
            Some(100),
            None,
        );
        pw.add_batch(50, 0);
        assert!(!pw.should_split(30, 0));
        assert!(pw.should_split(51, 0));
    }

    #[test]
    fn test_partition_writer_should_split_by_bytes() {
        let mut pw = PartitionWriter::new(
            PathBuf::from("out.csv"),
            None,
            Some(1024),
        );
        pw.add_batch(0, 500);
        assert!(!pw.should_split(0, 300));
        assert!(pw.should_split(0, 600));
    }
}
