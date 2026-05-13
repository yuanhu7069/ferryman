use std::time::Instant;
use indicatif::{ProgressBar, ProgressStyle};

pub struct ProgressTracker {
    bar: Option<ProgressBar>,
    start: Instant,
    total_rows: u64,
    processed_rows: u64,
    quiet: bool,
}

impl ProgressTracker {
    pub fn new(quiet: bool) -> Self {
        let bar = if quiet {
            None
        } else {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} rows | {msg}")
                    .unwrap()
                    .progress_chars("##-")
            );
            Some(pb)
        };

        ProgressTracker { bar, start: Instant::now(), total_rows: 0, processed_rows: 0, quiet }
    }

    pub fn set_total_rows(&mut self, total: u64) {
        self.total_rows = total;
        if let Some(ref bar) = self.bar {
            bar.set_length(total);
        }
    }

    pub fn update(&mut self, rows: usize) {
        self.processed_rows += rows as u64;
        if let Some(ref bar) = self.bar {
            bar.inc(rows as u64);
            let elapsed = self.start.elapsed().as_secs_f64();
            let rate = if elapsed > 0.0 { (self.processed_rows as f64 / elapsed) as u64 } else { 0 };
            let remaining = if rate > 0 && self.total_rows > 0 { (self.total_rows - self.processed_rows) / rate } else { 0 };
            bar.set_message(format!("{} rows/s | ETA {}s", rate, remaining));
        }
    }

    pub fn finish(&self) {
        if let Some(ref bar) = self.bar { bar.finish_and_clear(); }
        if !self.quiet {
            let elapsed = self.start.elapsed().as_secs_f64();
            let rate = if elapsed > 0.0 { (self.processed_rows as f64 / elapsed) as u64 } else { 0 };
            eprintln!("Done: {} rows in {:.1}s ({} rows/s)", self.processed_rows, elapsed, rate);
        }
    }
}
