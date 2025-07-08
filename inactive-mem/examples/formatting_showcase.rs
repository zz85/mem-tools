use linux_memory_monitor::*;

fn main() -> Result<()> {
    println!("Linux Memory Monitor - Formatting Showcase");
    println!("==========================================\n");

    let stats = MemoryStats::current()?;
    let pressure = MemoryPressure::current()?;

    println!("🎨 Beautiful Number Formatting Examples:");
    println!("----------------------------------------");

    // Show raw vs formatted numbers
    println!("Raw numbers vs Formatted:");
    println!(
        "  Total Memory:    {} → {}",
        stats.mem_total,
        format_memory_kb(stats.mem_total)
    );

    println!(
        "  Free Memory:     {} → {}",
        stats.mem_free,
        format_memory_kb(stats.mem_free)
    );

    println!(
        "  Page Cache:      {} → {}",
        stats.page_cache_size(),
        format_memory_kb(stats.page_cache_size())
    );

    println!(
        "  Inactive(file):  {} → {}",
        stats.inactive_file,
        format_memory_kb(stats.inactive_file)
    );

    println!("\n📊 Complete System Overview:");
    println!("----------------------------");
    println!("  Total Memory:      {}", format_memory_kb(stats.mem_total));
    println!(
        "  Available Memory:  {} ({:.1}%)",
        format_memory_kb(stats.mem_available),
        pressure.available_ratio * 100.0
    );
    println!("  Free Memory:       {}", format_memory_kb(stats.mem_free));
    println!(
        "  Used Memory:       {}",
        format_memory_kb(stats.used_memory())
    );

    println!("\n💾 Page Cache Details:");
    println!(
        "  Total Page Cache:  {}",
        format_memory_kb(stats.page_cache_size())
    );
    println!("    - Cached:        {}", format_memory_kb(stats.cached));
    println!("    - Buffers:       {}", format_memory_kb(stats.buffers));
    println!(
        "  Active(file):      {}",
        format_memory_kb(stats.active_file)
    );
    println!(
        "  Inactive(file):    {} ({}% of total)",
        format_memory_kb(stats.inactive_file),
        format_percentage(pressure.inactive_file_ratio)
    );

    println!("\n🔄 I/O Activity:");
    println!("  Dirty Pages:       {}", format_memory_kb(stats.dirty));
    println!("  Writeback:         {}", format_memory_kb(stats.writeback));

    println!("\n🧠 Kernel Memory:");
    println!("  Slab Total:        {}", format_memory_kb(stats.slab));
    println!(
        "    - Reclaimable:   {}",
        format_memory_kb(stats.s_reclaimable)
    );
    println!(
        "    - Unreclaimable: {}",
        format_memory_kb(stats.s_unreclaimable)
    );

    println!("\n📈 Memory Ratios:");
    println!(
        "  Memory Utilization: {}",
        format_percentage(stats.memory_utilization() / 100.0)
    );
    println!(
        "  Cache Utilization:  {}",
        format_percentage(pressure.cache_ratio)
    );
    println!(
        "  Dirty Ratio:        {}",
        format_percentage(pressure.dirty_ratio)
    );
    println!(
        "  Available Ratio:    {}",
        format_percentage(pressure.available_ratio)
    );

    // Demonstrate change formatting
    println!("\n🔄 Simulated Memory Changes:");
    println!(
        "  Large increase:     {}",
        format_memory_change_kb(1_500_000)
    ); // +1.5GB
    println!("  Medium increase:    {}", format_memory_change_kb(50_000)); // +50MB
    println!("  Small increase:     {}", format_memory_change_kb(1_024)); // +1MB
    println!("  Small decrease:     {}", format_memory_change_kb(-512)); // -512KB
    println!("  Medium decrease:    {}", format_memory_change_kb(-25_000)); // -25MB
    println!(
        "  Large decrease:     {}",
        format_memory_change_kb(-2_000_000)
    ); // -2GB

    println!("\n✨ The formatting makes large numbers much easier to read!");
    println!(
        "   Compare: {} vs {}",
        stats.mem_total,
        format_number(stats.mem_total)
    );

    Ok(())
}
