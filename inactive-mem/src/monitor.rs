use crate::{MemorySnapshot, MemoryStats, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Continuous memory monitor with configurable sampling
pub struct ContinuousMonitor {
    snapshots: Arc<Mutex<VecDeque<MemorySnapshot>>>,
    max_snapshots: usize,
    running: Arc<Mutex<bool>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ContinuousMonitor {
    /// Create a new continuous monitor
    pub fn new(max_snapshots: usize) -> Self {
        ContinuousMonitor {
            snapshots: Arc::new(Mutex::new(VecDeque::with_capacity(max_snapshots))),
            max_snapshots,
            running: Arc::new(Mutex::new(false)),
            handle: None,
        }
    }

    /// Start monitoring with specified interval
    pub fn start(&mut self, interval: Duration) -> Result<()> {
        let mut running = self.running.lock().unwrap();
        if *running {
            return Ok(()); // Already running
        }
        *running = true;

        let snapshots = Arc::clone(&self.snapshots);
        let running_flag = Arc::clone(&self.running);
        let max_snapshots = self.max_snapshots;

        let handle = thread::spawn(move || {
            while *running_flag.lock().unwrap() {
                if let Ok(snapshot) = MemorySnapshot::new() {
                    let mut snapshots_guard = snapshots.lock().unwrap();
                    
                    // Add new snapshot
                    snapshots_guard.push_back(snapshot);
                    
                    // Remove old snapshots if we exceed the limit
                    while snapshots_guard.len() > max_snapshots {
                        snapshots_guard.pop_front();
                    }
                }
                
                thread::sleep(interval);
            }
        });

        self.handle = Some(handle);
        Ok(())
    }

    /// Stop monitoring
    pub fn stop(&mut self) {
        {
            let mut running = self.running.lock().unwrap();
            *running = false;
        }

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Get current snapshots
    pub fn get_snapshots(&self) -> Vec<MemorySnapshot> {
        self.snapshots.lock().unwrap().iter().cloned().collect()
    }

    /// Get latest snapshot
    pub fn get_latest(&self) -> Option<MemorySnapshot> {
        self.snapshots.lock().unwrap().back().cloned()
    }

    /// Get memory trend analysis
    pub fn get_trend_analysis(&self, window_size: usize) -> Option<TrendAnalysis> {
        let snapshots = self.snapshots.lock().unwrap();
        if snapshots.len() < window_size {
            return None;
        }

        let recent: Vec<_> = snapshots.iter().rev().take(window_size).cloned().collect();
        Some(TrendAnalysis::from_snapshots(&recent))
    }

    /// Clear all stored snapshots
    pub fn clear(&self) {
        self.snapshots.lock().unwrap().clear();
    }
}

impl Drop for ContinuousMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Memory trend analysis over a time window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub duration_ms: u64,
    pub sample_count: usize,
    pub memory_trends: MemoryTrends,
    pub cache_trends: CacheTrends,
    pub pressure_changes: Vec<f64>, // Available memory ratio over time
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryTrends {
    pub free_memory_trend: Trend,
    pub used_memory_trend: Trend,
    pub available_memory_trend: Trend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheTrends {
    pub page_cache_trend: Trend,
    pub inactive_file_trend: Trend,
    pub active_file_trend: Trend,
    pub dirty_pages_trend: Trend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trend {
    pub initial_value: u64,
    pub final_value: u64,
    pub change: i64,
    pub change_percent: f64,
    pub direction: TrendDirection,
    pub volatility: f64, // Standard deviation of changes
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
}

impl TrendAnalysis {
    fn from_snapshots(snapshots: &[MemorySnapshot]) -> Self {
        if snapshots.is_empty() {
            return Self::default();
        }

        let first = &snapshots[0];
        let last = snapshots.last().unwrap();
        let duration_ms = last.timestamp.saturating_sub(first.timestamp);

        // Calculate trends for different memory metrics
        let free_values: Vec<u64> = snapshots.iter().map(|s| s.stats.mem_free).collect();
        let used_values: Vec<u64> = snapshots.iter().map(|s| s.stats.used_memory()).collect();
        let available_values: Vec<u64> = snapshots.iter().map(|s| s.stats.mem_available).collect();
        let cache_values: Vec<u64> = snapshots.iter().map(|s| s.stats.page_cache_size()).collect();
        let inactive_file_values: Vec<u64> = snapshots.iter().map(|s| s.stats.inactive_file).collect();
        let active_file_values: Vec<u64> = snapshots.iter().map(|s| s.stats.active_file).collect();
        let dirty_values: Vec<u64> = snapshots.iter().map(|s| s.stats.dirty).collect();

        let memory_trends = MemoryTrends {
            free_memory_trend: Self::calculate_trend(&free_values),
            used_memory_trend: Self::calculate_trend(&used_values),
            available_memory_trend: Self::calculate_trend(&available_values),
        };

        let cache_trends = CacheTrends {
            page_cache_trend: Self::calculate_trend(&cache_values),
            inactive_file_trend: Self::calculate_trend(&inactive_file_values),
            active_file_trend: Self::calculate_trend(&active_file_values),
            dirty_pages_trend: Self::calculate_trend(&dirty_values),
        };

        let pressure_changes: Vec<f64> = snapshots.iter()
            .map(|s| s.stats.mem_available as f64 / s.stats.mem_total as f64)
            .collect();

        TrendAnalysis {
            duration_ms,
            sample_count: snapshots.len(),
            memory_trends,
            cache_trends,
            pressure_changes,
        }
    }

    fn calculate_trend(values: &[u64]) -> Trend {
        if values.is_empty() {
            return Trend::default();
        }

        let initial_value = values[0];
        let final_value = *values.last().unwrap();
        let change = final_value as i64 - initial_value as i64;
        let change_percent = if initial_value > 0 {
            (change as f64 / initial_value as f64) * 100.0
        } else {
            0.0
        };

        let direction = match change {
            c if c > (initial_value as i64 / 100) => TrendDirection::Increasing, // > 1% change
            c if c < -(initial_value as i64 / 100) => TrendDirection::Decreasing, // < -1% change
            _ => TrendDirection::Stable,
        };

        // Calculate volatility (standard deviation of changes)
        let volatility = if values.len() > 1 {
            let changes: Vec<f64> = values.windows(2)
                .map(|w| (w[1] as f64 - w[0] as f64).abs())
                .collect();
            let mean_change = changes.iter().sum::<f64>() / changes.len() as f64;
            let variance = changes.iter()
                .map(|&x| (x - mean_change).powi(2))
                .sum::<f64>() / changes.len() as f64;
            variance.sqrt()
        } else {
            0.0
        };

        Trend {
            initial_value,
            final_value,
            change,
            change_percent,
            direction,
            volatility,
        }
    }
}

impl Default for TrendAnalysis {
    fn default() -> Self {
        Self {
            duration_ms: 0,
            sample_count: 0,
            memory_trends: MemoryTrends {
                free_memory_trend: Trend::default(),
                used_memory_trend: Trend::default(),
                available_memory_trend: Trend::default(),
            },
            cache_trends: CacheTrends {
                page_cache_trend: Trend::default(),
                inactive_file_trend: Trend::default(),
                active_file_trend: Trend::default(),
                dirty_pages_trend: Trend::default(),
            },
            pressure_changes: Vec::new(),
        }
    }
}

impl Default for Trend {
    fn default() -> Self {
        Self {
            initial_value: 0,
            final_value: 0,
            change: 0,
            change_percent: 0.0,
            direction: TrendDirection::Stable,
            volatility: 0.0,
        }
    }
}

/// Event-based monitoring for specific memory conditions
pub struct EventMonitor {
    conditions: Vec<MemoryCondition>,
    last_snapshot: Option<MemorySnapshot>,
}

pub struct MemoryCondition {
    pub name: String,
    pub condition: Box<dyn Fn(&MemoryStats, Option<&MemoryStats>) -> bool + Send + Sync>,
    pub triggered: bool,
}

impl std::fmt::Debug for MemoryCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryCondition")
            .field("name", &self.name)
            .field("triggered", &self.triggered)
            .field("condition", &"<function>")
            .finish()
    }
}

impl EventMonitor {
    pub fn new() -> Self {
        EventMonitor {
            conditions: Vec::new(),
            last_snapshot: None,
        }
    }

    /// Add a condition to monitor
    pub fn add_condition<F>(&mut self, name: String, condition: F)
    where
        F: Fn(&MemoryStats, Option<&MemoryStats>) -> bool + Send + Sync + 'static,
    {
        self.conditions.push(MemoryCondition {
            name,
            condition: Box::new(condition),
            triggered: false,
        });
    }

    /// Check all conditions against current memory state
    pub fn check_conditions(&mut self) -> Result<Vec<String>> {
        let current = MemorySnapshot::new()?;
        let mut triggered_events = Vec::new();

        let previous_stats = self.last_snapshot.as_ref().map(|s| &s.stats);

        for condition in &mut self.conditions {
            let is_triggered = (condition.condition)(&current.stats, previous_stats);
            
            if is_triggered && !condition.triggered {
                triggered_events.push(condition.name.clone());
                condition.triggered = true;
            } else if !is_triggered {
                condition.triggered = false;
            }
        }

        self.last_snapshot = Some(current);
        Ok(triggered_events)
    }

    /// Add common memory conditions
    pub fn add_common_conditions(&mut self) {
        // Low memory condition (< 10% available)
        self.add_condition(
            "low_memory".to_string(),
            |stats, _| (stats.mem_available as f64 / stats.mem_total as f64) < 0.1,
        );

        // High page cache growth (> 100MB increase)
        self.add_condition(
            "high_cache_growth".to_string(),
            |stats, prev| {
                if let Some(prev_stats) = prev {
                    let current_cache = stats.page_cache_size();
                    let prev_cache = prev_stats.page_cache_size();
                    current_cache > prev_cache + 100 * 1024 // 100MB in KB
                } else {
                    false
                }
            },
        );

        // High dirty pages (> 5% of total memory)
        self.add_condition(
            "high_dirty_pages".to_string(),
            |stats, _| (stats.dirty as f64 / stats.mem_total as f64) > 0.05,
        );

        // Memory pressure relief (available memory increased by > 50MB)
        self.add_condition(
            "memory_pressure_relief".to_string(),
            |stats, prev| {
                if let Some(prev_stats) = prev {
                    stats.mem_available > prev_stats.mem_available + 50 * 1024 // 50MB in KB
                } else {
                    false
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_continuous_monitor_creation() {
        let monitor = ContinuousMonitor::new(100);
        assert_eq!(monitor.max_snapshots, 100);
    }

    #[test]
    fn test_trend_calculation() {
        let values = vec![1000, 1100, 1200, 1150, 1300];
        let trend = TrendAnalysis::calculate_trend(&values);
        
        assert_eq!(trend.initial_value, 1000);
        assert_eq!(trend.final_value, 1300);
        assert_eq!(trend.change, 300);
        assert!(matches!(trend.direction, TrendDirection::Increasing));
    }

    #[test]
    fn test_event_monitor() {
        let mut monitor = EventMonitor::new();
        
        monitor.add_condition(
            "test_condition".to_string(),
            |stats, _| stats.mem_free < 1000,
        );

        // This test would need actual memory stats to be meaningful
        // In a real scenario, you'd mock the MemorySnapshot::new() function
    }
}
