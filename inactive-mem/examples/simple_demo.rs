use linux_memory_monitor::*;
use std::fs::File;
use std::io::Write;

fn main() -> Result<()> {
    println!("Simple Linux Memory Monitor Demo");
    println!("================================\n");

    // Show initial memory state
    let initial_stats = MemoryStats::current()?;
    println!("Initial Memory State:");
    println!("  Free Memory:      {:>10} KB", initial_stats.mem_free);
    println!("  Page Cache:       {:>10} KB", initial_stats.page_cache_size());
    println!("  Inactive(file):   {:>10} KB", initial_stats.inactive_file);
    println!("  Dirty Pages:      {:>10} KB", initial_stats.dirty);

    // Create a page cache monitor
    let mut monitor = PageCacheMonitor::new()?;

    // Write a 50MB file and observe memory impact
    println!("\nWriting 50MB file...");
    let file_data = vec![42u8; 50 * 1024 * 1024]; // 50MB of data
    
    let analysis = monitor.analyze_file_operation(|| {
        let mut file = File::create("/tmp/demo_file.dat")?;
        file.write_all(&file_data)?;
        file.sync_all()?;
        Ok(())
    })?;

    println!("File write completed!");
    println!("Memory impact: {}", analysis.summary());

    // Show what happened
    if analysis.caused_cache_growth() {
        println!("✅ Page cache grew as expected (file data cached)");
    }

    if analysis.memory_impact.inactive_file_change_kb > 0 {
        println!("✅ Inactive(file) increased by {} KB", 
                 analysis.memory_impact.inactive_file_change_kb);
        println!("   This is reclaimable memory that Linux can free if needed");
    }

    if analysis.memory_impact.dirty_change_kb > 0 {
        println!("✅ Dirty pages increased by {} KB", 
                 analysis.memory_impact.dirty_change_kb);
        println!("   These pages will be written to disk in the background");
    }

    // Show final memory state
    let final_stats = MemoryStats::current()?;
    println!("\nFinal Memory State:");
    println!("  Free Memory:      {:>10} KB (change: {:+} KB)", 
             final_stats.mem_free, 
             final_stats.mem_free as i64 - initial_stats.mem_free as i64);
    println!("  Page Cache:       {:>10} KB (change: {:+} KB)", 
             final_stats.page_cache_size(),
             final_stats.page_cache_size() as i64 - initial_stats.page_cache_size() as i64);
    println!("  Inactive(file):   {:>10} KB (change: {:+} KB)", 
             final_stats.inactive_file,
             final_stats.inactive_file as i64 - initial_stats.inactive_file as i64);
    println!("  Dirty Pages:      {:>10} KB (change: {:+} KB)", 
             final_stats.dirty,
             final_stats.dirty as i64 - initial_stats.dirty as i64);

    // Clean up
    let _ = std::fs::remove_file("/tmp/demo_file.dat");

    println!("\n🎯 Key Insights:");
    println!("   • File writes increase page cache (cached + buffers)");
    println!("   • New file pages typically start as Inactive(file)");
    println!("   • Inactive(file) pages are easily reclaimable by the kernel");
    println!("   • This is how Linux optimizes file I/O performance!");

    Ok(())
}
