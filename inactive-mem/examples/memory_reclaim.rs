use linux_memory_monitor::*;
use std::fs::File;
use std::io::Write;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    println!("Memory Reclamation Demo");
    println!("======================\n");

    // Show initial state
    let initial = MemoryStats::current()?;
    println!("Initial State:");
    println!("  Inactive(file): {:>10} KB", initial.inactive_file);
    println!("  Free Memory:    {:>10} KB", initial.mem_free);

    // Create several large files to consume memory
    println!("\nCreating large files to fill page cache...");
    let file_paths = ["/tmp/big1.dat", "/tmp/big2.dat", "/tmp/big3.dat"];
    
    for (i, path) in file_paths.iter().enumerate() {
        let size_mb = (i + 1) * 20; // 20MB, 40MB, 60MB
        let data = vec![i as u8; size_mb * 1024 * 1024];
        
        if let Ok(mut file) = File::create(path) {
            let _ = file.write_all(&data);
            let _ = file.sync_all();
            println!("  Created {} ({} MB)", path, size_mb);
        }
    }

    // Check memory after file creation
    let after_files = MemoryStats::current()?;
    println!("\nAfter creating files:");
    println!("  Inactive(file): {:>10} KB (change: {:+} KB)", 
             after_files.inactive_file,
             after_files.inactive_file as i64 - initial.inactive_file as i64);
    println!("  Free Memory:    {:>10} KB (change: {:+} KB)", 
             after_files.mem_free,
             after_files.mem_free as i64 - initial.mem_free as i64);

    // Now delete the files (but memory should still be cached)
    println!("\nDeleting files (but memory stays cached)...");
    for path in &file_paths {
        let _ = std::fs::remove_file(path);
    }

    let after_delete = MemoryStats::current()?;
    println!("After deleting files:");
    println!("  Inactive(file): {:>10} KB (change: {:+} KB)", 
             after_delete.inactive_file,
             after_delete.inactive_file as i64 - after_files.inactive_file as i64);
    println!("  Free Memory:    {:>10} KB (change: {:+} KB)", 
             after_delete.mem_free,
             after_delete.mem_free as i64 - after_files.mem_free as i64);

    println!("\n💡 Notice: Deleting files doesn't immediately free the page cache!");
    println!("   The kernel keeps the data cached in case you need it again.");

    // Try to trigger memory reclamation by creating memory pressure
    println!("\nTrying to create memory pressure to trigger reclamation...");
    
    // Create a large allocation to put pressure on memory
    println!("  Allocating large vector to create memory pressure...");
    let _large_vec: Vec<u8> = vec![0; 100 * 1024 * 1024]; // 100MB
    
    // Wait a moment for the kernel to react
    thread::sleep(Duration::from_millis(500));
    
    let after_pressure = MemoryStats::current()?;
    println!("After memory pressure:");
    println!("  Inactive(file): {:>10} KB (change: {:+} KB)", 
             after_pressure.inactive_file,
             after_pressure.inactive_file as i64 - after_delete.inactive_file as i64);
    println!("  Free Memory:    {:>10} KB (change: {:+} KB)", 
             after_pressure.mem_free,
             after_pressure.mem_free as i64 - after_delete.mem_free as i64);

    // Show memory pressure analysis
    let pressure = MemoryPressure::from_stats(&after_pressure);
    println!("\nMemory Pressure Analysis:");
    println!("  Available Memory: {:.1}%", pressure.available_ratio * 100.0);
    println!("  Pressure Level:   {:?}", pressure.pressure_level);
    println!("  Inactive(file):   {:.1}% of total memory", pressure.inactive_file_ratio * 100.0);

    println!("\n🎯 Key Insights:");
    println!("   • Linux keeps file data cached even after files are deleted");
    println!("   • Inactive(file) pages are the first to be reclaimed under pressure");
    println!("   • This provides excellent I/O performance with automatic cleanup");
    println!("   • You can monitor this behavior to understand your system's memory usage");

    Ok(())
}
