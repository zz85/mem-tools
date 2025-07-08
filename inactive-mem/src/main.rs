use linux_memory_monitor::*;
use std::fs::File;
use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    println!("Linux Memory Monitor - Continuous Inactive Memory Generation");
    println!("===========================================================\n");

    let interval = Duration::from_secs(3); // Stats interval (not used for pausing anymore)
    let mut file_counter = 0;
    let file_size_gb = 1; // Create 1GB files each time
    let max_files = 20; // Maximum number of files to create before cleanup
    let target_inactive_gb = 50; // Stop after generating this much new inactive memory

    println!("Configuration:");
    println!("  File size: {} GB per file", file_size_gb);
    println!("  No pause between files - running at maximum speed!");
    println!("  Max files before cleanup: {}", max_files);
    println!(
        "  Will stop after generating {} GB of new inactive memory\n",
        target_inactive_gb
    );

    // Show initial state
    let initial_stats = MemoryStats::current()?;
    let initial_inactive_gb = initial_stats.inactive_file as f64 / (1024.0 * 1024.0);
    print_memory_stats("INITIAL STATE", &initial_stats);

    let start_time = Instant::now();
    let mut created_files = Vec::new();
    let mut total_new_inactive = 0.0;

    loop {
        // Create a large file to generate inactive memory
        let file_path = format!("/tmp/inactive_mem_test_{}.dat", file_counter);
        println!("\n🔄 Creating file: {} ({} GB)", file_path, file_size_gb);

        let create_start = Instant::now();
        match create_large_file(&file_path, file_size_gb) {
            Ok(_) => {
                let create_duration = create_start.elapsed();
                println!(
                    "✅ File created in {:.2} seconds",
                    create_duration.as_secs_f64()
                );
                created_files.push(file_path.clone());
                file_counter += 1;
            }
            Err(e) => {
                println!("❌ Failed to create file: {}", e);
                break;
            }
        }

        // Print current memory stats
        let current_stats = MemoryStats::current()?;
        print_memory_stats(&format!("AFTER FILE #{}", file_counter), &current_stats);

        // Calculate progress
        let current_inactive_gb = current_stats.inactive_file as f64 / (1024.0 * 1024.0);
        total_new_inactive = current_inactive_gb - initial_inactive_gb;
        let total_runtime = start_time.elapsed();

        println!("\n📊 PROGRESS SUMMARY:");
        println!(
            "  Runtime: {:.1} minutes",
            total_runtime.as_secs_f64() / 60.0
        );
        println!("  Files created: {}", file_counter);
        println!(
            "  Total file data written: {} GB",
            file_counter * file_size_gb
        );
        println!("  Initial inactive(file): {:.1} GB", initial_inactive_gb);
        println!("  Current inactive(file): {:.1} GB", current_inactive_gb);
        println!("  🎯 NEW inactive memory: {:.1} GB", total_new_inactive);
        println!(
            "  Inactive memory ratio: {:.1}%",
            current_stats.inactive_file as f64 / current_stats.mem_total as f64 * 100.0
        );

        // Check if we've reached our target
        if total_new_inactive >= target_inactive_gb as f64 {
            println!("\n🎉 TARGET ACHIEVED!");
            println!(
                "   Generated {:.1} GB of new inactive file memory!",
                total_new_inactive
            );
            println!("   This demonstrates Linux's page cache behavior at scale.");
            break;
        }

        // Check if we should clean up old files
        if created_files.len() >= max_files {
            println!("\n🧹 Cleaning up oldest files to prevent disk space issues...");
            let files_to_remove = created_files.len() - (max_files / 2);
            for _ in 0..files_to_remove {
                if !created_files.is_empty() {
                    let old_file = created_files.remove(0);
                    if let Err(e) = std::fs::remove_file(&old_file) {
                        println!("⚠️  Failed to remove {}: {}", old_file, e);
                    } else {
                        println!("🗑️  Removed: {}", old_file);
                    }
                }
            }

            // Show memory stats after cleanup
            thread::sleep(Duration::from_millis(500)); // Let kernel react
            let after_cleanup = MemoryStats::current()?;
            print_memory_stats("AFTER CLEANUP", &after_cleanup);
        }

        // Check for memory pressure
        let pressure = MemoryPressure::from_stats(&current_stats);
        match pressure.pressure_level {
            PressureLevel::High | PressureLevel::Critical => {
                println!("\n⚠️  HIGH MEMORY PRESSURE DETECTED!");
                println!("   Available: {:.1}%", pressure.available_ratio * 100.0);
                println!("   Stopping to prevent system issues.");
                break;
            }
            PressureLevel::Medium => {
                println!("\n⚡ Medium memory pressure - continuing with caution");
                thread::sleep(Duration::from_secs(2));
            }
            PressureLevel::Low => {
                // Continue at normal pace
            }
        }

        // Continue immediately to next file creation
        println!("\n🔄 Continuing to next file...");
    }

    // Final summary
    let final_stats = MemoryStats::current()?;
    let final_inactive_gb = final_stats.inactive_file as f64 / (1024.0 * 1024.0);
    let total_runtime = start_time.elapsed();

    println!("\n{}", "=".repeat(60));
    println!("🏁 FINAL SUMMARY");
    println!("{}", "=".repeat(60));
    println!(
        "Total runtime: {:.1} minutes",
        total_runtime.as_secs_f64() / 60.0
    );
    println!("Files created: {}", file_counter);
    println!("Total data written: {} GB", file_counter * file_size_gb);
    println!("Initial inactive(file): {:.1} GB", initial_inactive_gb);
    println!("Final inactive(file): {:.1} GB", final_inactive_gb);
    println!(
        "🎯 Net inactive memory generated: {:.1} GB",
        final_inactive_gb - initial_inactive_gb
    );
    println!(
        "Average file creation time: {:.2} seconds",
        total_runtime.as_secs_f64() / file_counter as f64
    );

    // Cleanup on exit
    println!("\n🧹 Cleaning up all test files...");
    for file_path in created_files {
        if let Err(e) = std::fs::remove_file(&file_path) {
            println!("⚠️  Failed to remove {}: {}", file_path, e);
        }
    }
    println!("✅ Cleanup complete!");

    Ok(())
}

fn create_large_file(path: &str, size_gb: usize) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    let chunk_size = 64 * 1024 * 1024; // 64MB chunks for better performance
    let chunk = vec![0u8; chunk_size];
    let chunks_per_gb = (1024 * 1024 * 1024) / chunk_size;
    let total_chunks = size_gb * chunks_per_gb;

    for i in 0..total_chunks {
        file.write_all(&chunk)?;

        // Sync every 8 chunks (512MB) to avoid too much dirty memory
        if i % 8 == 7 {
            file.sync_data()?;
        }
    }

    file.sync_all()?;
    Ok(())
}

fn print_memory_stats(label: &str, stats: &MemoryStats) {
    println!("\n📊 {} - Memory Statistics:", label);
    println!("  ┌─────────────────────────────────────────────────────────────┐");
    println!(
        "  │ Total Memory:      {} │",
        format!("{:>35}", format_memory_kb(stats.mem_total))
    );
    println!(
        "  │ Free Memory:       {} │",
        format!("{:>35}", format_memory_kb(stats.mem_free))
    );
    println!(
        "  │ Available Memory:  {} │",
        format!("{:>35}", format_memory_kb(stats.mem_available))
    );
    println!(
        "  │ Page Cache:        {} │",
        format!("{:>35}", format_memory_kb(stats.page_cache_size()))
    );
    println!("  │ ──────────────────────────────────────────────────────────── │");
    println!(
        "  │ 🎯 Inactive(file): {} │",
        format!("{:>35}", format_memory_kb(stats.inactive_file))
    );
    println!(
        "  │ Active(file):      {} │",
        format!("{:>35}", format_memory_kb(stats.active_file))
    );
    println!("  │ ──────────────────────────────────────────────────────────── │");
    println!(
        "  │ Dirty Pages:       {} │",
        format!("{:>35}", format_memory_kb(stats.dirty))
    );
    println!(
        "  │ Writeback:         {} │",
        format!("{:>35}", format_memory_kb(stats.writeback))
    );
    println!("  └─────────────────────────────────────────────────────────────┘");

    // Calculate and show key ratios
    let inactive_ratio = stats.inactive_file as f64 / stats.mem_total as f64 * 100.0;
    let cache_ratio = stats.page_cache_size() as f64 / stats.mem_total as f64 * 100.0;
    let available_ratio = stats.mem_available as f64 / stats.mem_total as f64 * 100.0;

    println!("  📈 Key Ratios:");
    println!(
        "     Inactive(file): {:.1}% of total memory",
        inactive_ratio
    );
    println!("     Page Cache:     {:.1}% of total memory", cache_ratio);
    println!(
        "     Available:      {:.1}% of total memory",
        available_ratio
    );
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
