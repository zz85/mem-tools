use linux_memory_monitor::*;
use std::fs::File;
use std::io::Write;

fn main() -> Result<()> {
    println!("Simple Linux Memory Monitor Demo");
    println!("================================\n");

    // Show initial memory state
    let initial_stats = MemoryStats::current()?;
    println!("Initial Memory State:");
    println!(
        "  Free Memory:      {:>15} KB",
        format_number(initial_stats.mem_free)
    );
    println!(
        "  Page Cache:       {:>15} KB",
        format_number(initial_stats.page_cache_size())
    );
    println!(
        "  Inactive(file):   {:>15} KB",
        format_number(initial_stats.inactive_file)
    );
    println!(
        "  Dirty Pages:      {:>15} KB",
        format_number(initial_stats.dirty)
    );

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
    println!("Memory impact: {}", format_analysis_summary(&analysis));

    // Show what happened
    if analysis.caused_cache_growth() {
        println!("✅ Page cache grew as expected (file data cached)");
    }

    if analysis.memory_impact.inactive_file_change_kb > 0 {
        println!(
            "✅ Inactive(file) increased by {} KB",
            format_number(analysis.memory_impact.inactive_file_change_kb as u64)
        );
        println!("   This is reclaimable memory that Linux can free if needed");
    }

    if analysis.memory_impact.dirty_change_kb > 0 {
        println!(
            "✅ Dirty pages increased by {} KB",
            format_number(analysis.memory_impact.dirty_change_kb as u64)
        );
        println!("   These pages will be written to disk in the background");
    }

    // Show final memory state
    let final_stats = MemoryStats::current()?;
    println!("\nFinal Memory State:");
    println!(
        "  Free Memory:      {:>15} KB (change: {} KB)",
        format_number(final_stats.mem_free),
        format_signed_number(final_stats.mem_free as i64 - initial_stats.mem_free as i64)
    );
    println!(
        "  Page Cache:       {:>15} KB (change: {} KB)",
        format_number(final_stats.page_cache_size()),
        format_signed_number(
            final_stats.page_cache_size() as i64 - initial_stats.page_cache_size() as i64
        )
    );
    println!(
        "  Inactive(file):   {:>15} KB (change: {} KB)",
        format_number(final_stats.inactive_file),
        format_signed_number(final_stats.inactive_file as i64 - initial_stats.inactive_file as i64)
    );
    println!(
        "  Dirty Pages:      {:>15} KB (change: {} KB)",
        format_number(final_stats.dirty),
        format_signed_number(final_stats.dirty as i64 - initial_stats.dirty as i64)
    );

    // Clean up
    let _ = std::fs::remove_file("/tmp/demo_file.dat");

    println!("\n🎯 Key Insights:");
    println!("   • File writes increase page cache (cached + buffers)");
    println!("   • New file pages typically start as Inactive(file)");
    println!("   • Inactive(file) pages are easily reclaimable by the kernel");
    println!("   • This is how Linux optimizes file I/O performance!");

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

/// Format analysis summary with comma-separated numbers
fn format_analysis_summary(analysis: &FileOperationAnalysis) -> String {
    format!(
        "Operation took {:?} | Cache: {}KB | Free: {}KB | Inactive(file): {}KB | Dirty: {}KB",
        analysis.operation_duration,
        format_signed_number(analysis.memory_impact.cache_change_kb),
        format_signed_number(analysis.memory_impact.free_memory_change_kb),
        format_signed_number(analysis.memory_impact.inactive_file_change_kb),
        format_signed_number(analysis.memory_impact.dirty_change_kb)
    )
}
