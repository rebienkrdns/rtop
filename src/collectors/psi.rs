use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use crate::models::{PsiData, PsiValues};

pub struct PsiCollector {
    base_path: PathBuf,
}

impl Default for PsiCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl PsiCollector {
    pub fn new() -> Self {
        Self {
            base_path: PathBuf::from("/proc/pressure"),
        }
    }

    #[cfg(test)]
    pub fn with_base_path<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            base_path: path.into(),
        }
    }

    pub fn collect(&self) -> Option<PsiData> {
        // Under non-Linux systems, only run if base_path is not "/proc/pressure" (e.g. in tests)
        #[cfg(not(target_os = "linux"))]
        {
            if self.base_path == Path::new("/proc/pressure") {
                return None;
            }
        }

        // Parse CPU pressure (only has "some")
        let cpu_path = self.base_path.join("cpu");
        let (cpu_some, _) = Self::parse_file(&cpu_path).ok()?;
        let cpu_some = cpu_some?;

        // Parse Memory pressure (has "some" and "full")
        let mem_path = self.base_path.join("memory");
        let (mem_some, mem_full) = Self::parse_file(&mem_path).ok()?;
        let mem_some = mem_some?;
        let mem_full = mem_full?;

        // Parse IO pressure (has "some" and "full")
        let io_path = self.base_path.join("io");
        let (io_some, io_full) = Self::parse_file(&io_path).ok()?;
        let io_some = io_some?;
        let io_full = io_full?;

        Some(PsiData {
            cpu_some,
            memory_some: mem_some,
            memory_full: mem_full,
            io_some,
            io_full,
        })
    }

    fn parse_file(path: &Path) -> std::io::Result<(Option<PsiValues>, Option<PsiValues>)> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut some_val = None;
        let mut full_val = None;

        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            let kind = parts[0];
            let mut avg10 = 0.0;
            let mut avg60 = 0.0;
            let mut avg300 = 0.0;
            let mut total = 0;

            for part in &parts[1..] {
                let kv: Vec<&str> = part.split('=').collect();
                if kv.len() != 2 {
                    continue;
                }
                let key = kv[0];
                let val = kv[1];
                match key {
                    "avg10" => avg10 = val.parse::<f64>().unwrap_or(0.0),
                    "avg60" => avg60 = val.parse::<f64>().unwrap_or(0.0),
                    "avg300" => avg300 = val.parse::<f64>().unwrap_or(0.0),
                    "total" => total = val.parse::<u64>().unwrap_or(0),
                    _ => {}
                }
            }

            let values = PsiValues { avg10, avg60, avg300, total };
            if kind == "some" {
                some_val = Some(values);
            } else if kind == "full" {
                full_val = Some(values);
            }
        }

        Ok((some_val, full_val))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, write, remove_dir_all};

    #[test]
    fn test_psi_collector_success() {
        let test_dir = PathBuf::from("target/test_psi_success");
        let _ = remove_dir_all(&test_dir);
        create_dir_all(&test_dir).unwrap();

        // Write mock CPU
        write(
            test_dir.join("cpu"),
            "some avg10=1.23 avg60=4.56 avg300=7.89 total=123456\n"
        ).unwrap();

        // Write mock memory
        write(
            test_dir.join("memory"),
            "some avg10=0.01 avg60=0.05 avg300=0.10 total=999\nfull avg10=2.34 avg60=5.67 avg300=8.90 total=555\n"
        ).unwrap();

        // Write mock io
        write(
            test_dir.join("io"),
            "some avg10=10.00 avg60=20.00 avg300=30.00 total=1000\nfull avg10=5.00 avg60=6.00 avg300=7.00 total=500\n"
        ).unwrap();

        let collector = PsiCollector::with_base_path(&test_dir);
        let data = collector.collect().expect("Should parse successfully");

        assert_eq!(data.cpu_some.avg10, 1.23);
        assert_eq!(data.cpu_some.avg60, 4.56);
        assert_eq!(data.cpu_some.avg300, 7.89);
        assert_eq!(data.cpu_some.total, 123456);

        assert_eq!(data.memory_some.avg10, 0.01);
        assert_eq!(data.memory_some.avg60, 0.05);
        assert_eq!(data.memory_some.avg300, 0.10);
        assert_eq!(data.memory_some.total, 999);
        assert_eq!(data.memory_full.avg10, 2.34);
        assert_eq!(data.memory_full.avg60, 5.67);
        assert_eq!(data.memory_full.avg300, 8.90);
        assert_eq!(data.memory_full.total, 555);

        assert_eq!(data.io_some.avg10, 10.00);
        assert_eq!(data.io_some.avg60, 20.00);
        assert_eq!(data.io_some.avg300, 30.00);
        assert_eq!(data.io_some.total, 1000);
        assert_eq!(data.io_full.avg10, 5.00);
        assert_eq!(data.io_full.avg60, 6.00);
        assert_eq!(data.io_full.avg300, 7.00);
        assert_eq!(data.io_full.total, 500);

        remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_psi_collector_missing_files() {
        let test_dir = PathBuf::from("target/test_psi_missing");
        let _ = remove_dir_all(&test_dir);
        create_dir_all(&test_dir).unwrap();

        // CPU exists but Memory is missing
        write(
            test_dir.join("cpu"),
            "some avg10=1.23 avg60=4.56 avg300=7.89 total=123456\n"
        ).unwrap();

        let collector = PsiCollector::with_base_path(&test_dir);
        let data = collector.collect();
        assert!(data.is_none(), "Should fail if any file is missing");

        remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_psi_collector_fallback_on_mac() {
        // When using the default path, it should return None on non-Linux
        #[cfg(not(target_os = "linux"))]
        {
            let collector = PsiCollector::new();
            let data = collector.collect();
            assert!(data.is_none());
        }
    }
}
