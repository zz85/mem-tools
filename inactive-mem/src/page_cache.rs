use crate::{MemorySnapshot, MemoryStats, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

/// Page cache monitoring and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageCacheMonitor {
    pub initial_snapshot: MemorySnapshot,
    pub snapshots: Vec<MemorySnapshot>,
}

impl PageCacheMonitor {
    /// Create a new page cache monitor
    pub fn new() -> Result<Self> {
        let initial_snapshot = MemorySnapshot::new()?;
        Ok(PageCacheMonitor {
            initial_snapshot: initial_snapshot.clone(),
            snapshots: vec![initial_snapshot],
        })
    }

    /// Take a new snapshot and add it to the monitoring history
    pub fn take_snapshot(&mut self) -> Result<&MemorySnapshot> {
        let snapshot = MemorySnapshot::new()?;
        self.snapshots.push(snapshot);
        Ok(self.snapshots.last().unwrap())
    }

    /// Get the latest snapshot
    pub fn latest_snapshot(&self) -> &MemorySnapshot {
        self.snapshots.last().unwrap()
    }

    /// Analyze page cache behavior during file operations
    pub fn analyze_file_operation<F>(&mut self, operation: F) -> Result<FileOperationAnalysis>
    where
        F: FnOnce() -> io::Result<()>,
    {
        // Take snapshot before operation
        let before = MemorySnapshot::new()?;
        
        // Perform the operation
        let start_time = Instant::now();
        operation().map_err(|e| crate::MemoryError::ProcMemInfoRead(e))?;
        let operation_duration = start_time.elapsed();
        
        // Take snapshot after operation
        let after = MemorySnapshot::new()?;
        
        // Add snapshots to history
        self.snapshots.push(before.clone());
        self.snapshots.push(after.clone());
        
        Ok(FileOperationAnalysis::new(before, after, operation_duration))
    }

    /// Monitor page cache behavior over time
    pub fn monitor_for_duration(&mut self, duration: Duration, interval: Duration) -> Result<Vec<MemorySnapshot>> {
        let mut snapshots = Vec::new();
        let start = Instant::now();
        
        while start.elapsed() < duration {
            let snapshot = MemorySnapshot::new()?;
            snapshots.push(snapshot.clone());
            self.snapshots.push(snapshot);
            
            std::thread::sleep(interval);
        }
        
        Ok(snapshots)
    }

    /// Get page cache statistics summary
    pub fn get_cache_summary(&self) -> PageCacheSummary {
        if self.snapshots.is_empty() {
            return PageCacheSummary::default();
        }

        let first = &self.snapshots[0];
        let last = self.snapshots.last().unwrap();
        
        let initial_cache = first.stats.page_cache_size();
        let final_cache = last.stats.page_cache_size();
        let cache_change = final_cache as i64 - initial_cache as i64;
        
        let max_cache = self.snapshots.iter()
            .map(|s| s.stats.page_cache_size())
            .max()
            .unwrap_or(0);
            
        let min_cache = self.snapshots.iter()
            .map(|s| s.stats.page_cache_size())
            .min()
            .unwrap_or(0);

        let max_inactive_file = self.snapshots.iter()
            .map(|s| s.stats.inactive_file)
            .max()
            .unwrap_or(0);

        PageCacheSummary {
            initial_cache_kb: initial_cache,
            final_cache_kb: final_cache,
            cache_change_kb: cache_change,
            max_cache_kb: max_cache,
            min_cache_kb: min_cache,
            max_inactive_file_kb: max_inactive_file,
            snapshot_count: self.snapshots.len(),
        }
    }
}

/// Analysis of file operation impact on memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperationAnalysis {
    pub before: MemorySnapshot,
    pub after: MemorySnapshot,
    pub operation_duration: Duration,
    pub memory_impact: MemoryImpact,
}

impl FileOperationAnalysis {
    fn new(before: MemorySnapshot, after: MemorySnapshot, operation_duration: Duration) -> Self {
        let memory_impact = MemoryImpact::calculate(&before.stats, &after.stats);
        
        FileOperationAnalysis {
            before,
            after,
            operation_duration,
            memory_impact,
        }
    }

    /// Check if the operation caused significant page cache growth
    pub fn caused_cache_growth(&self) -> bool {
        self.memory_impact.cache_change_kb > 1024 // More than 1MB
    }

    /// Check if memory was freed after the operation
    pub fn freed_memory(&self) -> bool {
        self.memory_impact.free_memory_change_kb > 0
    }

    /// Get a summary of the operation's impact
    pub fn summary(&self) -> String {
        format!(
            "Operation took {:?} | Cache: {:+}KB | Free: {:+}KB | Inactive(file): {:+}KB | Dirty: {:+}KB",
            self.operation_duration,
            self.memory_impact.cache_change_kb,
            self.memory_impact.free_memory_change_kb,
            self.memory_impact.inactive_file_change_kb,
            self.memory_impact.dirty_change_kb
        )
    }
}

/// Memory impact analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryImpact {
    pub free_memory_change_kb: i64,
    pub cache_change_kb: i64,
    pub inactive_file_change_kb: i64,
    pub active_file_change_kb: i64,
    pub dirty_change_kb: i64,
    pub writeback_change_kb: i64,
}

impl MemoryImpact {
    fn calculate(before: &MemoryStats, after: &MemoryStats) -> Self {
        MemoryImpact {
            free_memory_change_kb: after.mem_free as i64 - before.mem_free as i64,
            cache_change_kb: after.page_cache_size() as i64 - before.page_cache_size() as i64,
            inactive_file_change_kb: after.inactive_file as i64 - before.inactive_file as i64,
            active_file_change_kb: after.active_file as i64 - before.active_file as i64,
            dirty_change_kb: after.dirty as i64 - before.dirty as i64,
            writeback_change_kb: after.writeback as i64 - before.writeback as i64,
        }
    }
}

/// Summary of page cache behavior over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageCacheSummary {
    pub initial_cache_kb: u64,
    pub final_cache_kb: u64,
    pub cache_change_kb: i64,
    pub max_cache_kb: u64,
    pub min_cache_kb: u64,
    pub max_inactive_file_kb: u64,
    pub snapshot_count: usize,
}

impl Default for PageCacheSummary {
    fn default() -> Self {
        Self {
            initial_cache_kb: 0,
            final_cache_kb: 0,
            cache_change_kb: 0,
            max_cache_kb: 0,
            min_cache_kb: 0,
            max_inactive_file_kb: 0,
            snapshot_count: 0,
        }
    }
}

/// File operation utilities for testing page cache behavior
pub struct FileOperations;

impl FileOperations {
    /// Write data to a file and analyze memory impact
    pub fn write_file_and_analyze<P: AsRef<Path>>(
        monitor: &mut PageCacheMonitor,
        path: P,
        data: &[u8],
    ) -> Result<FileOperationAnalysis> {
        monitor.analyze_file_operation(|| {
            let mut file = File::create(path)?;
            file.write_all(data)?;
            file.sync_all()?;
            Ok(())
        })
    }

    /// Read a file and analyze memory impact
    pub fn read_file_and_analyze<P: AsRef<Path>>(
        monitor: &mut PageCacheMonitor,
        path: P,
    ) -> Result<FileOperationAnalysis> {
        monitor.analyze_file_operation(|| {
            std::fs::read(path)?;
            Ok(())
        })
    }

    /// Create a large file for testing
    pub fn create_test_file<P: AsRef<Path>>(path: P, size_mb: usize) -> io::Result<()> {
        let mut file = File::create(path)?;
        let chunk = vec![0u8; 1024 * 1024]; // 1MB chunk
        
        for _ in 0..size_mb {
            file.write_all(&chunk)?;
        }
        
        file.sync_all()?;
        Ok(())
    }

    /// Force file data to be written to disk
    pub fn sync_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
        let file = File::open(path)?;
        file.sync_all()?;
        Ok(())
    }

    /// Remove a file from the filesystem
    pub fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
        std::fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_page_cache_monitor_creation() {
        let monitor = PageCacheMonitor::new();
        assert!(monitor.is_ok());
        
        let monitor = monitor.unwrap();
        assert_eq!(monitor.snapshots.len(), 1);
    }

    #[test]
    fn test_memory_impact_calculation() {
        let before = MemoryStats {
            mem_free: 1000000,
            cached: 500000,
            buffers: 100000,
            inactive_file: 300000,
            dirty: 50000,
            ..Default::default()
        };

        let after = MemoryStats {
            mem_free: 800000,
            cached: 700000,
            buffers: 100000,
            inactive_file: 400000,
            dirty: 75000,
            ..Default::default()
        };

        let impact = MemoryImpact::calculate(&before, &after);
        assert_eq!(impact.free_memory_change_kb, -200000);
        assert_eq!(impact.cache_change_kb, 200000); // (700000 + 100000) - (500000 + 100000)
        assert_eq!(impact.inactive_file_change_kb, 100000);
        assert_eq!(impact.dirty_change_kb, 25000);
    }

    #[test]
    fn test_file_operations() -> io::Result<()> {
        let temp_file = NamedTempFile::new()?;
        let test_data = b"Hello, World! This is test data for page cache monitoring.";
        
        // Test file creation
        FileOperations::create_test_file(temp_file.path(), 1)?;
        
        // Verify file exists and has content
        let metadata = fs::metadata(temp_file.path())?;
        assert!(metadata.len() >= 1024 * 1024); // At least 1MB
        
        Ok(())
    }
}
