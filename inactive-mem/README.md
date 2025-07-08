# Linux Memory Monitor

A Rust crate for monitoring Linux memory metrics with a focus on page cache behavior and memory management analysis. This crate is particularly useful for understanding how file I/O operations affect system memory, page cache utilization, and memory pressure.

## Features

- **Real-time Memory Monitoring**: Track key Linux memory metrics from `/proc/meminfo`
- **Page Cache Analysis**: Monitor page cache behavior during file operations
- **Memory Pressure Detection**: Identify and analyze memory pressure conditions
- **Continuous Monitoring**: Long-term memory trend analysis
- **Event-based Monitoring**: Trigger alerts on specific memory conditions
- **File I/O Impact Analysis**: Understand how file operations affect memory
- **Human-readable Formatting**: Numbers displayed with comma separators and appropriate units

## Key Memory Metrics Tracked

- **MemFree**: Available free memory
- **MemAvailable**: Memory available for new processes
- **Cached**: Page cache memory
- **Buffers**: Buffer cache memory
- **Active(file)**: Recently used file-backed pages
- **Inactive(file)**: Less recently used file-backed pages (reclaimable)
- **Dirty**: Pages waiting to be written to disk
- **Writeback**: Pages currently being written to disk
- **Slab**: Kernel slab allocator memory

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
linux-memory-monitor = "0.1.0"
```

## Quick Start

### Basic Memory Information

```rust
use linux_memory_monitor::*;

fn main() -> Result<()> {
    // Get current memory statistics
    let stats = MemoryStats::current()?;
    
    println!("Total Memory: {} KB", stats.mem_total);
    println!("Free Memory: {} KB", stats.mem_free);
    println!("Page Cache: {} KB", stats.page_cache_size());
    println!("Inactive(file): {} KB", stats.inactive_file);
    
    // Analyze memory pressure
    let pressure = MemoryPressure::current()?;
    println!("Memory Pressure: {:?}", pressure.pressure_level);
    println!("Available: {:.1}%", pressure.available_ratio * 100.0);
    
    Ok(())
}
```

### Monitor File I/O Impact

```rust
use linux_memory_monitor::*;

fn main() -> Result<()> {
    let mut monitor = PageCacheMonitor::new()?;
    
    // Analyze the impact of writing a file
    let analysis = FileOperations::write_file_and_analyze(
        &mut monitor,
        "/tmp/test_file.dat",
        &vec![0u8; 10 * 1024 * 1024] // 10MB file
    )?;
    
    println!("File write impact: {}", analysis.summary());
    
    if analysis.caused_cache_growth() {
        println!("✅ File write increased page cache");
    }
    
    if analysis.freed_memory() {
        println!("✅ Memory was freed during operation");
    }
    
    Ok(())
}
```

### Continuous Memory Monitoring

```rust
use linux_memory_monitor::*;
use std::time::Duration;

fn main() -> Result<()> {
    let mut monitor = ContinuousMonitor::new(1000); // Keep 1000 snapshots
    
    // Start monitoring every 500ms
    monitor.start(Duration::from_millis(500))?;
    
    // Let it run for 30 seconds
    std::thread::sleep(Duration::from_secs(30));
    
    monitor.stop();
    
    // Analyze trends
    if let Some(trend) = monitor.get_trend_analysis(20) {
        println!("Memory trend over {} samples:", trend.sample_count);
        println!("Page cache: {} KB → {} KB ({:+} KB)",
                 trend.cache_trends.page_cache_trend.initial_value,
                 trend.cache_trends.page_cache_trend.final_value,
                 trend.cache_trends.page_cache_trend.change);
    }
    
    Ok(())
}
```

### Event-based Monitoring

```rust
use linux_memory_monitor::*;

fn main() -> Result<()> {
    let mut event_monitor = EventMonitor::new();
    
    // Add built-in conditions
    event_monitor.add_common_conditions();
    
    // Add custom condition
    event_monitor.add_condition(
        "high_page_cache".to_string(),
        |stats, _| {
            let cache_ratio = stats.page_cache_size() as f64 / stats.mem_total as f64;
            cache_ratio > 0.5 // Alert if page cache > 50% of total memory
        }
    );
    
    // Check conditions periodically
    loop {
        let events = event_monitor.check_conditions()?;
        
        for event in events {
            println!("🚨 Memory event triggered: {}", event);
        }
        
        std::thread::sleep(Duration::from_secs(5));
    }
}
```

## Understanding Page Cache Behavior

This crate is particularly useful for understanding Linux page cache behavior:

### What happens when you write a file?

1. **Dirty Pages Increase**: Data is written to memory first
2. **Page Cache Grows**: File data is cached in memory
3. **Inactive(file) May Increase**: New file pages start as inactive
4. **Writeback Activity**: Pages are eventually written to disk

### What happens when you read a file?

1. **Page Cache Grows**: File data is loaded into cache
2. **Active(file) Increases**: Recently accessed pages become active
3. **Memory Pressure**: May cause other pages to become inactive

### Memory Reclamation

When memory pressure occurs:
1. **Inactive(file) pages are reclaimed first** - these are the easiest to free
2. **Clean pages are dropped** - no need to write to disk
3. **Dirty pages are written back** - then freed

## Use Cases

- **Performance Analysis**: Understand memory bottlenecks in applications
- **System Monitoring**: Track memory health in production systems
- **File I/O Optimization**: Analyze the memory impact of file operations
- **Memory Leak Detection**: Monitor for unusual memory growth patterns
- **Cache Efficiency**: Understand page cache hit/miss patterns
- **System Tuning**: Optimize memory settings based on usage patterns

## Advanced Features

### Memory Utilities

```rust
use linux_memory_monitor::*;

// Force filesystem sync
MemoryUtils::sync_filesystem()?;

// Drop page caches (requires root)
MemoryUtils::drop_caches(3)?; // Drop all caches

// Get process memory info
let proc_info = MemoryUtils::process_memory_info(1234)?;
println!("Process RSS: {} KB", proc_info.vm_rss);
```

### Memory Snapshots and Diffs

```rust
use linux_memory_monitor::*;

let before = MemorySnapshot::new()?;

// ... perform some operation ...

let after = MemorySnapshot::new()?;
let diff = MemoryDiff::between(&before, &after);

println!("Memory change: {}", diff.format_summary());
```

## Platform Support

This crate is designed specifically for Linux systems and requires access to:
- `/proc/meminfo` - for memory statistics
- `/proc/sys/vm/drop_caches` - for cache management (optional, requires root)
- `/proc/PID/status` - for process memory info

## Performance

The crate is designed to be lightweight:
- Reading `/proc/meminfo` is fast (typically < 1ms)
- Memory snapshots are small and efficient
- Continuous monitoring has minimal overhead
- No external dependencies for core functionality

## Examples

Run the included examples:

```bash
# Basic memory monitoring
cargo run --example basic

# File I/O impact analysis
cargo run --example file_io

# Continuous monitoring
cargo run --example continuous

# Event monitoring
cargo run --example events
```

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Test specific module
cargo test memory::tests
```

## Contributing

Contributions are welcome! Please feel free to submit issues, feature requests, or pull requests.

## License

This project is licensed under either of:
- Apache License, Version 2.0
- MIT License

at your option.
