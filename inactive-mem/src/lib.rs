use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use thiserror::Error;

pub mod memory;
pub mod page_cache;
pub mod monitor;

pub use memory::*;
pub use page_cache::*;
pub use monitor::*;

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Failed to read /proc/meminfo: {0}")]
    ProcMemInfoRead(#[from] io::Error),
    #[error("Failed to parse memory value: {0}")]
    ParseError(String),
    #[error("Memory field not found: {0}")]
    FieldNotFound(String),
}

pub type Result<T> = std::result::Result<T, MemoryError>;

/// Core memory statistics from /proc/meminfo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total usable RAM (physical RAM minus reserved bits and kernel binary code)
    pub mem_total: u64,
    /// Amount of free memory
    pub mem_free: u64,
    /// Memory currently in use by the system
    pub mem_available: u64,
    /// Memory used by buffers
    pub buffers: u64,
    /// Memory used by page cache and slabs
    pub cached: u64,
    /// Swap cache memory
    pub swap_cached: u64,
    /// Memory that has been used more recently and usually not reclaimed unless absolutely necessary
    pub active: u64,
    /// Memory which has been less recently used and is more eligible to be reclaimed
    pub inactive: u64,
    /// Active memory for file-backed pages
    pub active_file: u64,
    /// Inactive memory for file-backed pages (page cache that can be reclaimed)
    pub inactive_file: u64,
    /// Active memory for anonymous pages
    pub active_anon: u64,
    /// Inactive memory for anonymous pages
    pub inactive_anon: u64,
    /// Memory that is waiting to be written back to disk
    pub dirty: u64,
    /// Memory that is actively being written back to disk
    pub writeback: u64,
    /// Memory mapped by mmap()
    pub mapped: u64,
    /// Shared memory
    pub shmem: u64,
    /// Kernel slab memory
    pub slab: u64,
    /// Reclaimable slab memory
    pub s_reclaimable: u64,
    /// Unreclaimable slab memory
    pub s_unreclaimable: u64,
}

impl MemoryStats {
    /// Read current memory statistics from /proc/meminfo
    pub fn current() -> Result<Self> {
        let content = fs::read_to_string("/proc/meminfo")?;
        Self::parse_meminfo(&content)
    }

    /// Parse /proc/meminfo content into MemoryStats
    fn parse_meminfo(content: &str) -> Result<Self> {
        let mut fields = HashMap::new();
        
        for line in content.lines() {
            if let Some((key, value_str)) = line.split_once(':') {
                let key = key.trim();
                let value_str = value_str.trim();
                
                // Extract numeric value (remove "kB" suffix if present)
                let value = if let Some(num_str) = value_str.split_whitespace().next() {
                    num_str.parse::<u64>()
                        .map_err(|_| MemoryError::ParseError(format!("Invalid number: {}", num_str)))?
                } else {
                    return Err(MemoryError::ParseError(format!("No value found for {}", key)));
                };
                
                fields.insert(key.to_string(), value);
            }
        }

        // Helper function to get field value
        let get_field = |name: &str| -> Result<u64> {
            fields.get(name)
                .copied()
                .ok_or_else(|| MemoryError::FieldNotFound(name.to_string()))
        };

        Ok(MemoryStats {
            mem_total: get_field("MemTotal")?,
            mem_free: get_field("MemFree")?,
            mem_available: get_field("MemAvailable")?,
            buffers: get_field("Buffers")?,
            cached: get_field("Cached")?,
            swap_cached: get_field("SwapCached")?,
            active: get_field("Active")?,
            inactive: get_field("Inactive")?,
            active_file: get_field("Active(file)")?,
            inactive_file: get_field("Inactive(file)")?,
            active_anon: get_field("Active(anon)")?,
            inactive_anon: get_field("Inactive(anon)")?,
            dirty: get_field("Dirty")?,
            writeback: get_field("Writeback")?,
            mapped: get_field("Mapped")?,
            shmem: get_field("Shmem")?,
            slab: get_field("Slab")?,
            s_reclaimable: get_field("SReclaimable")?,
            s_unreclaimable: get_field("SUnreclaim")?,
        })
    }

    /// Calculate used memory (Total - Free - Buffers - Cached)
    pub fn used_memory(&self) -> u64 {
        self.mem_total.saturating_sub(self.mem_free + self.buffers + self.cached)
    }

    /// Calculate page cache size (Cached + Buffers)
    pub fn page_cache_size(&self) -> u64 {
        self.cached + self.buffers
    }

    /// Calculate memory utilization percentage
    pub fn memory_utilization(&self) -> f64 {
        if self.mem_total == 0 {
            0.0
        } else {
            (self.used_memory() as f64 / self.mem_total as f64) * 100.0
        }
    }

    /// Calculate page cache utilization percentage
    pub fn page_cache_utilization(&self) -> f64 {
        if self.mem_total == 0 {
            0.0
        } else {
            (self.page_cache_size() as f64 / self.mem_total as f64) * 100.0
        }
    }

    /// Convert all values from KB to bytes
    pub fn to_bytes(&self) -> MemoryStats {
        MemoryStats {
            mem_total: self.mem_total * 1024,
            mem_free: self.mem_free * 1024,
            mem_available: self.mem_available * 1024,
            buffers: self.buffers * 1024,
            cached: self.cached * 1024,
            swap_cached: self.swap_cached * 1024,
            active: self.active * 1024,
            inactive: self.inactive * 1024,
            active_file: self.active_file * 1024,
            inactive_file: self.inactive_file * 1024,
            active_anon: self.active_anon * 1024,
            inactive_anon: self.inactive_anon * 1024,
            dirty: self.dirty * 1024,
            writeback: self.writeback * 1024,
            mapped: self.mapped * 1024,
            shmem: self.shmem * 1024,
            slab: self.slab * 1024,
            s_reclaimable: self.s_reclaimable * 1024,
            s_unreclaimable: self.s_unreclaimable * 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_meminfo() {
        let sample_meminfo = r#"MemTotal:       16384000 kB
MemFree:         8192000 kB
MemAvailable:   12288000 kB
Buffers:          512000 kB
Cached:          2048000 kB
SwapCached:            0 kB
Active:          4096000 kB
Inactive:        2048000 kB
Active(file):    1024000 kB
Inactive(file):  1536000 kB
Active(anon):    3072000 kB
Inactive(anon):   512000 kB
Dirty:             64000 kB
Writeback:             0 kB
Mapped:           256000 kB
Shmem:            128000 kB
Slab:             384000 kB
SReclaimable:     256000 kB
SUnreclaim:       128000 kB"#;

        let stats = MemoryStats::parse_meminfo(sample_meminfo).unwrap();
        assert_eq!(stats.mem_total, 16384000);
        assert_eq!(stats.mem_free, 8192000);
        assert_eq!(stats.cached, 2048000);
        assert_eq!(stats.inactive_file, 1536000);
    }

    #[test]
    fn test_memory_calculations() {
        let stats = MemoryStats {
            mem_total: 16384000,
            mem_free: 8192000,
            buffers: 512000,
            cached: 2048000,
            // ... other fields with default values for test
            ..Default::default()
        };

        assert_eq!(stats.used_memory(), 5632000); // 16384000 - 8192000 - 512000 - 2048000
        assert_eq!(stats.page_cache_size(), 2560000); // 2048000 + 512000
    }
}

// Implement Default for MemoryStats for testing
impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            mem_total: 0,
            mem_free: 0,
            mem_available: 0,
            buffers: 0,
            cached: 0,
            swap_cached: 0,
            active: 0,
            inactive: 0,
            active_file: 0,
            inactive_file: 0,
            active_anon: 0,
            inactive_anon: 0,
            dirty: 0,
            writeback: 0,
            mapped: 0,
            shmem: 0,
            slab: 0,
            s_reclaimable: 0,
            s_unreclaimable: 0,
        }
    }
}
