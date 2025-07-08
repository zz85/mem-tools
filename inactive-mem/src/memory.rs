use crate::{MemoryStats, Result};
use serde::{Deserialize, Serialize};

/// Memory snapshot with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub timestamp: u64, // Unix timestamp in milliseconds
    pub stats: MemoryStats,
}

impl MemorySnapshot {
    /// Create a new memory snapshot with current time and memory stats
    pub fn new() -> Result<Self> {
        let stats = MemoryStats::current()?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Ok(MemorySnapshot { timestamp, stats })
    }

    /// Create a snapshot with a specific timestamp (useful for testing)
    pub fn with_timestamp(timestamp: u64) -> Result<Self> {
        let stats = MemoryStats::current()?;
        Ok(MemorySnapshot { timestamp, stats })
    }
}

/// Memory difference between two snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDiff {
    pub duration_ms: u64,
    pub mem_free_diff: i64,
    pub cached_diff: i64,
    pub buffers_diff: i64,
    pub inactive_file_diff: i64,
    pub active_file_diff: i64,
    pub dirty_diff: i64,
    pub writeback_diff: i64,
    pub page_cache_diff: i64,
}

impl MemoryDiff {
    /// Calculate difference between two memory snapshots
    pub fn between(before: &MemorySnapshot, after: &MemorySnapshot) -> Self {
        let duration_ms = after.timestamp.saturating_sub(before.timestamp);
        
        MemoryDiff {
            duration_ms,
            mem_free_diff: after.stats.mem_free as i64 - before.stats.mem_free as i64,
            cached_diff: after.stats.cached as i64 - before.stats.cached as i64,
            buffers_diff: after.stats.buffers as i64 - before.stats.buffers as i64,
            inactive_file_diff: after.stats.inactive_file as i64 - before.stats.inactive_file as i64,
            active_file_diff: after.stats.active_file as i64 - before.stats.active_file as i64,
            dirty_diff: after.stats.dirty as i64 - before.stats.dirty as i64,
            writeback_diff: after.stats.writeback as i64 - before.stats.writeback as i64,
            page_cache_diff: (after.stats.page_cache_size() as i64) - (before.stats.page_cache_size() as i64),
        }
    }

    /// Check if memory was freed (positive value means more free memory)
    pub fn memory_was_freed(&self) -> bool {
        self.mem_free_diff > 0
    }

    /// Check if page cache increased (indicating file I/O activity)
    pub fn page_cache_increased(&self) -> bool {
        self.page_cache_diff > 0
    }

    /// Check if there was significant dirty page activity
    pub fn has_dirty_activity(&self) -> bool {
        self.dirty_diff.abs() > 1024 // More than 1MB change
    }

    /// Format the diff as a human-readable string
    pub fn format_summary(&self) -> String {
        format!(
            "Duration: {}ms | Free: {:+}KB | Cache: {:+}KB | Inactive(file): {:+}KB | Dirty: {:+}KB",
            self.duration_ms,
            self.mem_free_diff,
            self.cached_diff,
            self.inactive_file_diff,
            self.dirty_diff
        )
    }
}

/// Memory pressure indicators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPressure {
    pub available_ratio: f64,    // MemAvailable / MemTotal
    pub free_ratio: f64,         // MemFree / MemTotal
    pub cache_ratio: f64,        // (Cached + Buffers) / MemTotal
    pub dirty_ratio: f64,        // Dirty / MemTotal
    pub inactive_file_ratio: f64, // Inactive(file) / MemTotal
    pub pressure_level: PressureLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PressureLevel {
    Low,      // > 50% available
    Medium,   // 20-50% available
    High,     // 10-20% available
    Critical, // < 10% available
}

impl MemoryPressure {
    /// Calculate memory pressure from current stats
    pub fn from_stats(stats: &MemoryStats) -> Self {
        let available_ratio = stats.mem_available as f64 / stats.mem_total as f64;
        let free_ratio = stats.mem_free as f64 / stats.mem_total as f64;
        let cache_ratio = stats.page_cache_size() as f64 / stats.mem_total as f64;
        let dirty_ratio = stats.dirty as f64 / stats.mem_total as f64;
        let inactive_file_ratio = stats.inactive_file as f64 / stats.mem_total as f64;

        let pressure_level = match available_ratio {
            r if r > 0.5 => PressureLevel::Low,
            r if r > 0.2 => PressureLevel::Medium,
            r if r > 0.1 => PressureLevel::High,
            _ => PressureLevel::Critical,
        };

        MemoryPressure {
            available_ratio,
            free_ratio,
            cache_ratio,
            dirty_ratio,
            inactive_file_ratio,
            pressure_level,
        }
    }

    /// Get current memory pressure
    pub fn current() -> Result<Self> {
        let stats = MemoryStats::current()?;
        Ok(Self::from_stats(&stats))
    }
}

/// Utility functions for memory operations
pub struct MemoryUtils;

impl MemoryUtils {
    /// Force a sync to flush dirty pages to disk
    pub fn sync_filesystem() -> std::io::Result<()> {
        std::process::Command::new("sync").status()?;
        Ok(())
    }

    /// Drop page caches (requires root privileges)
    /// echo 1 > /proc/sys/vm/drop_caches  # Drop page cache
    /// echo 2 > /proc/sys/vm/drop_caches  # Drop dentries and inodes
    /// echo 3 > /proc/sys/vm/drop_caches  # Drop all caches
    pub fn drop_caches(cache_type: u8) -> std::io::Result<()> {
        if cache_type > 3 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cache type must be 1, 2, or 3",
            ));
        }

        std::fs::write("/proc/sys/vm/drop_caches", cache_type.to_string())
    }

    /// Get memory info for a specific process
    pub fn process_memory_info(pid: u32) -> std::io::Result<ProcessMemoryInfo> {
        let status_path = format!("/proc/{}/status", pid);
        let content = std::fs::read_to_string(status_path)?;
        
        let mut vm_rss = 0;
        let mut vm_size = 0;
        
        for line in content.lines() {
            if let Some(value_str) = line.strip_prefix("VmRSS:") {
                if let Some(num_str) = value_str.trim().split_whitespace().next() {
                    vm_rss = num_str.parse().unwrap_or(0);
                }
            } else if let Some(value_str) = line.strip_prefix("VmSize:") {
                if let Some(num_str) = value_str.trim().split_whitespace().next() {
                    vm_size = num_str.parse().unwrap_or(0);
                }
            }
        }

        Ok(ProcessMemoryInfo { vm_rss, vm_size })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMemoryInfo {
    pub vm_rss: u64,  // Resident Set Size in KB
    pub vm_size: u64, // Virtual Memory Size in KB
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_diff_calculation() {
        let before = MemorySnapshot {
            timestamp: 1000,
            stats: MemoryStats {
                mem_free: 1000000,
                cached: 500000,
                inactive_file: 300000,
                ..Default::default()
            },
        };

        let after = MemorySnapshot {
            timestamp: 2000,
            stats: MemoryStats {
                mem_free: 800000,
                cached: 700000,
                inactive_file: 400000,
                ..Default::default()
            },
        };

        let diff = MemoryDiff::between(&before, &after);
        assert_eq!(diff.duration_ms, 1000);
        assert_eq!(diff.mem_free_diff, -200000);
        assert_eq!(diff.cached_diff, 200000);
        assert_eq!(diff.inactive_file_diff, 100000);
        assert!(diff.page_cache_increased());
        assert!(!diff.memory_was_freed());
    }

    #[test]
    fn test_pressure_level_calculation() {
        let stats = MemoryStats {
            mem_total: 1000000,
            mem_available: 600000,
            ..Default::default()
        };

        let pressure = MemoryPressure::from_stats(&stats);
        assert!(matches!(pressure.pressure_level, PressureLevel::Low));
        assert_eq!(pressure.available_ratio, 0.6);
    }
}
