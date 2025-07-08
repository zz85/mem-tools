use linux_memory_monitor::*;
use std::fs::File;
use std::io::Write;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    println!("Linux Memory Monitor - Page Cache Analysis Tool");
    println!("===============================================\n");

    // Show current memory state
    show_current_memory_state()?;

    // Demonstrate file I/O impact on memory
    demonstrate_file_io_impact()?;

    // Monitor memory continuously
    demonstrate_continuous_monitoring()?;

    // Test memory pressure scenarios
    demonstrate_memory_pressure_analysis()?;

    Ok(())
}

fn show_current_memory_state() -> Result<()> {
    println!("📊 Current Memory State:");
    println!("-----------------------");
    
    let stats = MemoryStats::current()?;
    let pressure = MemoryPressure::current()?;
    
    println!("Total Memory:      {:>10} KB ({:.1} GB)", 
             stats.mem_total, stats.mem_total as f64 / 1024.0 / 1024.0);
    println!("Free Memory:       {:>10} KB ({:.1} GB)", 
             stats.mem_free, stats.mem_free as f64 / 1024.0 / 1024.0);
    println!("Available Memory:  {:>10} KB ({:.1} GB)", 
             stats.mem_available, stats.mem_available as f64 / 1024.0 / 1024.0);
    println!("Page Cache:        {:>10} KB ({:.1} GB)", 
             stats.page_cache_size(), stats.page_cache_size() as f64 / 1024.0 / 1024.0);
    println!("  - Cached:        {:>10} KB", stats.cached);
    println!("  - Buffers:       {:>10} KB", stats.buffers);
    println!("Inactive(file):    {:>10} KB ({:.1} GB)", 
             stats.inactive_file, stats.inactive_file as f64 / 1024.0 / 1024.0);
    println!("Active(file):      {:>10} KB ({:.1} GB)", 
             stats.active_file, stats.active_file as f64 / 1024.0 / 1024.0);
    println!("Dirty Pages:       {:>10} KB ({:.1} MB)", 
             stats.dirty, stats.dirty as f64 / 1024.0);
    println!("Writeback:         {:>10} KB", stats.writeback);
    
    println!("\n📈 Memory Pressure Analysis:");
    println!("Available Ratio:   {:.1}%", pressure.available_ratio * 100.0);
    println!("Cache Ratio:       {:.1}%", pressure.cache_ratio * 100.0);
    println!("Dirty Ratio:       {:.3}%", pressure.dirty_ratio * 100.0);
    println!("Pressure Level:    {:?}", pressure.pressure_level);
    
    println!();
    Ok(())
}

fn demonstrate_file_io_impact() -> Result<()> {
    println!("🔍 Demonstrating File I/O Impact on Memory:");
    println!("--------------------------------------------");
    
    let mut monitor = PageCacheMonitor::new()?;
    let test_file = "/tmp/memory_test_file.dat";
    
    // Create a test file (10MB)
    println!("Creating 10MB test file...");
    let analysis = FileOperations::write_file_and_analyze(
        &mut monitor, 
        test_file, 
        &vec![0u8; 10 * 1024 * 1024]
    )?;
    
    println!("Write operation impact: {}", analysis.summary());
    
    if analysis.caused_cache_growth() {
        println!("✅ File write caused page cache growth as expected");
    } else {
        println!("ℹ️  File write did not significantly impact page cache");
    }
    
    // Wait a moment and read the file
    thread::sleep(Duration::from_millis(500));
    
    println!("\nReading the test file...");
    let read_analysis = FileOperations::read_file_and_analyze(&mut monitor, test_file)?;
    println!("Read operation impact: {}", read_analysis.summary());
    
    // Clean up
    let _ = std::fs::remove_file(test_file);
    
    // Show cache summary
    let summary = monitor.get_cache_summary();
    println!("\n📋 Page Cache Summary:");
    println!("Initial cache: {} KB", summary.initial_cache_kb);
    println!("Final cache:   {} KB", summary.final_cache_kb);
    println!("Cache change:  {:+} KB", summary.cache_change_kb);
    println!("Max cache:     {} KB", summary.max_cache_kb);
    println!("Snapshots:     {}", summary.snapshot_count);
    
    println!();
    Ok(())
}

fn demonstrate_continuous_monitoring() -> Result<()> {
    println!("⏱️  Continuous Memory Monitoring (10 seconds):");
    println!("----------------------------------------------");
    
    let mut monitor = ContinuousMonitor::new(100);
    monitor.start(Duration::from_millis(500))?;
    
    println!("Monitoring started... Creating some memory activity");
    
    // Create some file I/O activity to observe
    let test_files = ["/tmp/test1.dat", "/tmp/test2.dat", "/tmp/test3.dat"];
    
    for (i, file_path) in test_files.iter().enumerate() {
        thread::sleep(Duration::from_secs(1));
        
        // Create files of different sizes
        let size = (i + 1) * 2 * 1024 * 1024; // 2MB, 4MB, 6MB
        let data = vec![i as u8; size];
        
        if let Ok(mut file) = File::create(file_path) {
            let _ = file.write_all(&data);
            let _ = file.sync_all();
            println!("Created file {} ({} MB)", file_path, size / 1024 / 1024);
        }
    }
    
    // Wait for more samples
    thread::sleep(Duration::from_secs(7));
    
    monitor.stop();
    
    // Analyze the trend
    if let Some(trend) = monitor.get_trend_analysis(10) {
        println!("\n📈 Trend Analysis (last 10 samples):");
        println!("Duration: {} ms", trend.duration_ms);
        println!("Samples: {}", trend.sample_count);
        
        println!("\nMemory Trends:");
        print_trend("Free Memory", &trend.memory_trends.free_memory_trend);
        print_trend("Page Cache", &trend.cache_trends.page_cache_trend);
        print_trend("Inactive(file)", &trend.cache_trends.inactive_file_trend);
        print_trend("Dirty Pages", &trend.cache_trends.dirty_pages_trend);
        
        println!("\nMemory Pressure Over Time:");
        for (i, &pressure) in trend.pressure_changes.iter().enumerate() {
            if i % 3 == 0 { // Show every 3rd sample to avoid clutter
                println!("  Sample {}: {:.1}% available", i, pressure * 100.0);
            }
        }
    }
    
    // Clean up test files
    for file_path in &test_files {
        let _ = std::fs::remove_file(file_path);
    }
    
    println!();
    Ok(())
}

fn demonstrate_memory_pressure_analysis() -> Result<()> {
    println!("🚨 Memory Pressure Event Monitoring:");
    println!("------------------------------------");
    
    let mut event_monitor = EventMonitor::new();
    event_monitor.add_common_conditions();
    
    // Add a custom condition for demonstration
    event_monitor.add_condition(
        "large_cache_change".to_string(),
        |stats, prev| {
            if let Some(prev_stats) = prev {
                let current_cache = stats.page_cache_size();
                let prev_cache = prev_stats.page_cache_size();
                (current_cache as i64 - prev_cache as i64).abs() > 50 * 1024 // 50MB change
            } else {
                false
            }
        },
    );
    
    println!("Monitoring for memory events...");
    
    // Check conditions a few times
    for i in 0..5 {
        thread::sleep(Duration::from_secs(1));
        
        let events = event_monitor.check_conditions()?;
        if !events.is_empty() {
            println!("🔔 Events triggered at check {}: {:?}", i + 1, events);
        } else {
            println!("✅ Check {}: No events triggered", i + 1);
        }
        
        // Show current memory pressure
        let pressure = MemoryPressure::current()?;
        println!("   Current pressure: {:?} ({:.1}% available)", 
                 pressure.pressure_level, pressure.available_ratio * 100.0);
    }
    
    println!();
    Ok(())
}

fn print_trend(name: &str, trend: &Trend) {
    println!("  {}: {} KB → {} KB ({:+} KB, {:.1}%, {:?})", 
             name,
             trend.initial_value,
             trend.final_value,
             trend.change,
             trend.change_percent,
             trend.direction);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stats_current() {
        let result = MemoryStats::current();
        assert!(result.is_ok());
        
        let stats = result.unwrap();
        assert!(stats.mem_total > 0);
        assert!(stats.mem_free <= stats.mem_total);
    }

    #[test]
    fn test_page_cache_monitor() {
        let result = PageCacheMonitor::new();
        assert!(result.is_ok());
        
        let monitor = result.unwrap();
        assert_eq!(monitor.snapshots.len(), 1);
    }

    #[test]
    fn test_memory_calculations() {
        let stats = MemoryStats {
            mem_total: 8000000,
            mem_free: 2000000,
            buffers: 500000,
            cached: 1500000,
            ..Default::default()
        };
        
        assert_eq!(stats.used_memory(), 4000000); // 8M - 2M - 0.5M - 1.5M
        assert_eq!(stats.page_cache_size(), 2000000); // 1.5M + 0.5M
        assert_eq!(stats.memory_utilization(), 50.0); // 4M / 8M * 100
    }
}
