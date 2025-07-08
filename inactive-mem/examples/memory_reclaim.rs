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
    println!(
        "  Inactive(file): {:>15} KB",
        format_number(initial.inactive_file)
    );
    println!(
        "  Free Memory:    {:>15} KB",
        format_number(initial.mem_free)
    );

    // Create several large files to consume memory
    println!("\nCreating large files to fill page cache...");
    let file_paths = ["/tmp/big1.dat", "/tmp/big2.dat", "/tmp/big3.dat"];

    for (i, path) in file_paths.iter().enumerate() {
        let size_mb = (i + 1) * 200; // 200MB, 400MB, 600MB
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
    println!(
        "  Inactive(file): {:>15} KB (change: {} KB)",
        format_number(after_files.inactive_file),
        format_signed_number(after_files.inactive_file as i64 - initial.inactive_file as i64)
    );
    println!(
        "  Free Memory:    {:>15} KB (change: {} KB)",
        format_number(after_files.mem_free),
        format_signed_number(after_files.mem_free as i64 - initial.mem_free as i64)
    );

    // Now delete the files (but memory should still be cached)
    println!("\nDeleting files (but memory stays cached)...");
    for path in &file_paths {
        let _ = std::fs::remove_file(path);
    }

    let after_delete = MemoryStats::current()?;
    println!("After deleting files:");
    println!(
        "  Inactive(file): {:>15} KB (change: {} KB)",
        format_number(after_delete.inactive_file),
        format_signed_number(after_delete.inactive_file as i64 - after_files.inactive_file as i64)
    );
    println!(
        "  Free Memory:    {:>15} KB (change: {} KB)",
        format_number(after_delete.mem_free),
        format_signed_number(after_delete.mem_free as i64 - after_files.mem_free as i64)
    );

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
    println!(
        "  Inactive(file): {:>15} KB (change: {} KB)",
        format_number(after_pressure.inactive_file),
        format_signed_number(
            after_pressure.inactive_file as i64 - after_delete.inactive_file as i64
        )
    );
    println!(
        "  Free Memory:    {:>15} KB (change: {} KB)",
        format_number(after_pressure.mem_free),
        format_signed_number(after_pressure.mem_free as i64 - after_delete.mem_free as i64)
    );

    // Show memory pressure analysis
    let pressure = MemoryPressure::from_stats(&after_pressure);
    println!("\nMemory Pressure Analysis:");
    println!(
        "  Available Memory: {:.1}%",
        pressure.available_ratio * 100.0
    );
    println!("  Pressure Level:   {:?}", pressure.pressure_level);
    println!(
        "  Inactive(file):   {:.1}% of total memory",
        pressure.inactive_file_ratio * 100.0
    );

    println!("\n🎯 Key Insights:");
    println!("   • Linux keeps file data cached even after files are deleted");
    println!("   • Inactive(file) pages are the first to be reclaimed under pressure");
    println!("   • This provides excellent I/O performance with automatic cleanup");
    println!("   • You can monitor this behavior to understand your system's memory usage");

    Ok(())
}

/// Format a number with comma separators
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }

    result
}

/// Format a signed number with comma separators
fn format_signed_number(n: i64) -> String {
    if n >= 0 {
        format!("+{}", format_number(n as u64))
    } else {
        format!("-{}", format_number((-n) as u64))
    }
}
