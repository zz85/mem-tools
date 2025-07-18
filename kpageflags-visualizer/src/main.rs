use byteorder::{LittleEndian, ReadBytesExt};
use clap::{Arg, Command};
use colored::*;
use memmap2::MmapOptions;
use rand::Rng;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod tui;

// Helper function to estimate total pages from /proc/meminfo
fn get_estimated_total_pages() -> Result<u64, Box<dyn std::error::Error>> {
    let file = std::fs::File::open("/proc/meminfo")?;
    let reader = std::io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        if line.starts_with("MemTotal:") {
            // Extract the number (in kB)
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let mem_kb: u64 = parts[1].parse()?;
                // Convert kB to pages (assuming 4KB pages)
                let total_pages = (mem_kb * 1024) / 4096;
                return Ok(total_pages);
            }
        }
    }

    // Fallback: assume 4GB of memory
    Ok(1048576) // 4GB / 4KB = 1M pages
}

// Page flag definitions with categories
pub const PAGE_FLAGS: &[(u64, &str, &str, FlagCategory)] = &[
    (1 << 0, "LOCKED", "Page is locked", FlagCategory::State),
    (1 << 1, "ERROR", "Page has error", FlagCategory::Error),
    (
        1 << 2,
        "REFERENCED",
        "Page has been referenced",
        FlagCategory::Usage,
    ),
    (
        1 << 3,
        "UPTODATE",
        "Page is up to date",
        FlagCategory::State,
    ),
    (1 << 4, "DIRTY", "Page is dirty", FlagCategory::State),
    (1 << 5, "LRU", "Page is on LRU list", FlagCategory::Memory),
    (
        1 << 6,
        "ACTIVE",
        "Page is on active list",
        FlagCategory::Memory,
    ),
    (
        1 << 7,
        "SLAB",
        "Page is slab allocated",
        FlagCategory::Allocation,
    ),
    (
        1 << 8,
        "WRITEBACK",
        "Page is under writeback",
        FlagCategory::IO,
    ),
    (
        1 << 9,
        "RECLAIM",
        "Page is being reclaimed",
        FlagCategory::Memory,
    ),
    (
        1 << 10,
        "BUDDY",
        "Page is free buddy page",
        FlagCategory::Allocation,
    ),
    (
        1 << 11,
        "MMAP",
        "Page is memory mapped",
        FlagCategory::Usage,
    ),
    (1 << 12, "ANON", "Page is anonymous", FlagCategory::Usage),
    (
        1 << 13,
        "SWAPCACHE",
        "Page is in swap cache",
        FlagCategory::Memory,
    ),
    (
        1 << 14,
        "SWAPBACKED",
        "Page is swap backed",
        FlagCategory::Memory,
    ),
    (
        1 << 15,
        "COMPOUND_HEAD",
        "Compound page head",
        FlagCategory::Structure,
    ),
    (
        1 << 16,
        "COMPOUND_TAIL",
        "Compound page tail",
        FlagCategory::Structure,
    ),
    (1 << 17, "HUGE", "Huge page", FlagCategory::Structure),
    (
        1 << 18,
        "UNEVICTABLE",
        "Page is unevictable",
        FlagCategory::Memory,
    ),
    (
        1 << 19,
        "HWPOISON",
        "Hardware poisoned page",
        FlagCategory::Error,
    ),
    (
        1 << 20,
        "NOPAGE",
        "No page frame exists",
        FlagCategory::State,
    ),
    (1 << 21, "KSM", "KSM page", FlagCategory::Special),
    (
        1 << 22,
        "THP",
        "Transparent huge page",
        FlagCategory::Structure,
    ),
    (1 << 23, "OFFLINE", "Page is offline", FlagCategory::State),
    (1 << 24, "ZERO_PAGE", "Zero page", FlagCategory::Special),
    (1 << 25, "IDLE", "Page is idle", FlagCategory::Usage),
    (1 << 26, "PGTABLE", "Page table page", FlagCategory::Special),
    // Additional flags that might be present
    (
        1 << 32,
        "RESERVED",
        "Reserved page (common in early memory)",
        FlagCategory::State,
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlagCategory {
    State,      // Page state flags
    Memory,     // Memory management flags
    Usage,      // Usage tracking flags
    Allocation, // Allocation type flags
    IO,         // I/O related flags
    Structure,  // Page structure flags
    Special,    // Special purpose flags
    Error,      // Error flags
}

#[derive(Debug, Clone)]
pub struct PageInfo {
    pfn: u64,
    flags: u64,
}

impl PageInfo {
    fn new(pfn: u64, flags: u64) -> Self {
        Self { pfn, flags }
    }

    fn get_flag_names(&self) -> Vec<&'static str> {
        PAGE_FLAGS
            .iter()
            .filter(|(flag, _, _, _)| self.flags & flag != 0)
            .map(|(_, name, _, _)| *name)
            .collect()
    }

    fn get_flag_descriptions(&self) -> Vec<(&'static str, &'static str)> {
        PAGE_FLAGS
            .iter()
            .filter(|(flag, _, _, _)| self.flags & flag != 0)
            .map(|(_, name, desc, _)| (*name, *desc))
            .collect()
    }

    fn get_primary_category(&self) -> Option<FlagCategory> {
        // Return the category of the most significant flag
        PAGE_FLAGS
            .iter()
            .filter(|(flag, _, _, _)| self.flags & flag != 0)
            .map(|(_, _, _, category)| *category)
            .next()
    }

    fn get_flag_categories(&self) -> Vec<FlagCategory> {
        let mut categories: Vec<FlagCategory> = PAGE_FLAGS
            .iter()
            .filter(|(flag, _, _, _)| self.flags & flag != 0)
            .map(|(_, _, _, category)| *category)
            .collect();
        categories.sort_by_key(|c| format!("{:?}", c));
        categories.dedup();
        categories
    }

    fn get_unknown_flags(&self) -> Vec<u8> {
        let known_flags: u64 = PAGE_FLAGS.iter().map(|(flag, _, _, _)| flag).sum();
        let unknown_flags = self.flags & !known_flags;

        let mut unknown_bits = Vec::new();
        for bit in 0..64 {
            if unknown_flags & (1 << bit) != 0 {
                unknown_bits.push(bit);
            }
        }
        unknown_bits
    }
}

pub struct KPageFlagsReader {
    file: BufReader<File>,
}

impl KPageFlagsReader {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open("/proc/kpageflags")?;
        Ok(Self {
            file: BufReader::new(file),
        })
    }

    fn get_total_pages(&mut self) -> Result<u64, Box<dyn std::error::Error>> {
        // For /proc/kpageflags, we can't reliably get the file size
        // Instead, we'll return a flag value that indicates "read until EOF"
        Ok(u64::MAX) // Special value indicating "read all"
    }

    fn read_all_pages(
        &mut self,
        start_pfn: u64,
        interrupt_flag: Arc<AtomicBool>,
    ) -> Result<Vec<PageInfo>, Box<dyn std::error::Error>> {
        let mut pages = Vec::new();
        let mut pfn = start_pfn;
        let mut consecutive_failures = 0;
        const MAX_CONSECUTIVE_FAILURES: u32 = 1000;

        // Get estimated total for progress reporting
        let estimated_total = get_estimated_total_pages().unwrap_or(1048576);
        println!(
            "Reading all available pages starting from PFN 0x{:x}...",
            start_pfn
        );
        println!(
            "Estimated total pages in system: ~{}",
            estimated_total.to_string().cyan()
        );
        println!(
            "{}",
            "Press Ctrl-C to stop and show summary of pages scanned so far".yellow()
        );

        loop {
            // Check for interrupt signal every 1000 pages
            if pages.len() % 1000 == 0 && interrupt_flag.load(Ordering::Relaxed) {
                println!(
                    "\n{}",
                    "Interrupt received! Stopping scan and showing summary..."
                        .yellow()
                        .bold()
                );
                break;
            }

            match self.read_page_flags(pfn) {
                Ok(Some(flags)) => {
                    pages.push(PageInfo::new(pfn, flags));
                    consecutive_failures = 0;

                    // Show progress every 50,000 pages
                    if pages.len() % 50000 == 0 {
                        let progress = if estimated_total > 0 {
                            format!(
                                " ({:.1}%)",
                                (pages.len() as f64 / estimated_total as f64) * 100.0
                            )
                        } else {
                            String::new()
                        };
                        println!(
                            "Read {} pages so far{}",
                            pages.len().to_string().green(),
                            progress.yellow()
                        );
                    }
                }
                Ok(None) => {
                    consecutive_failures += 1;
                    if consecutive_failures > MAX_CONSECUTIVE_FAILURES {
                        break;
                    }
                }
                Err(_) => {
                    consecutive_failures += 1;
                    if consecutive_failures > MAX_CONSECUTIVE_FAILURES {
                        break;
                    }
                }
            }
            pfn += 1;

            // Safety check: don't read more than 100M pages (400GB of memory)
            if pages.len() > 100_000_000 {
                println!(
                    "{}",
                    "Warning: Reached safety limit of 100M pages. Stopping.".yellow()
                );
                break;
            }
        }

        let status_msg = if interrupt_flag.load(Ordering::Relaxed) {
            format!("Scan interrupted - successfully read {} pages", pages.len())
        } else {
            format!("Successfully read {} total pages", pages.len())
        };

        println!("{}", status_msg.green().bold());
        Ok(pages)
    }

    fn read_page_flags(&mut self, pfn: u64) -> Result<Option<u64>, Box<dyn std::error::Error>> {
        let offset = pfn * 8; // Each entry is 8 bytes
        self.file.seek(SeekFrom::Start(offset))?;

        match self.file.read_u64::<LittleEndian>() {
            Ok(flags) => Ok(Some(flags)),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(Box::new(e)),
        }
    }

    pub fn read_range(
        &mut self,
        start_pfn: u64,
        count: u64,
        interrupt_flag: Arc<AtomicBool>,
    ) -> Result<Vec<PageInfo>, Box<dyn std::error::Error>> {
        let mut pages = Vec::new();
        let mut consecutive_failures = 0;
        const MAX_CONSECUTIVE_FAILURES: u32 = 1000; // Stop after 1000 consecutive failures

        for pfn in start_pfn..start_pfn + count {
            // Check for interrupt signal every 1000 pages
            if pages.len() % 1000 == 0 && interrupt_flag.load(Ordering::Relaxed) {
                println!(
                    "\n{}",
                    "Interrupt received! Stopping scan and showing summary..."
                        .yellow()
                        .bold()
                );
                break;
            }

            match self.read_page_flags(pfn) {
                Ok(Some(flags)) => {
                    pages.push(PageInfo::new(pfn, flags));
                    consecutive_failures = 0;
                }
                Ok(None) => {
                    consecutive_failures += 1;
                    if consecutive_failures > MAX_CONSECUTIVE_FAILURES {
                        // We've hit the end of available pages
                        break;
                    }
                }
                Err(_) => {
                    consecutive_failures += 1;
                    if consecutive_failures > MAX_CONSECUTIVE_FAILURES {
                        break;
                    }
                }
            }
        }

        Ok(pages)
    }

    /// Optimized summary-only scan that minimizes allocations
    /// Only stores counters, not individual PageInfo objects
    pub fn scan_for_summary_only(
        &mut self,
        start_pfn: u64,
        count: Option<u64>,
        interrupt_flag: Arc<AtomicBool>,
        show_histogram: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Pre-allocate fixed-size arrays for counters to avoid HashMap allocations
        const MAX_FLAGS: usize = PAGE_FLAGS.len();
        let mut flag_counts = [0u32; MAX_FLAGS];
        let mut category_counts = [0u32; 8]; // 8 categories in FlagCategory enum

        let mut total_pages = 0u32;
        let mut pages_with_flags = 0u32;
        let mut pfn = start_pfn;
        let mut consecutive_failures = 0u32;
        const MAX_CONSECUTIVE_FAILURES: u32 = 1000;

        let estimated_total = if count.is_none() {
            get_estimated_total_pages().unwrap_or(1048576)
        } else {
            count.unwrap()
        };

        println!(
            "Scanning pages for summary (optimized mode) starting from PFN 0x{:x}...",
            start_pfn
        );

        if count.is_none() {
            println!(
                "Estimated total pages in system: ~{}",
                estimated_total.to_string().cyan()
            );
            println!(
                "{}",
                "Press Ctrl-C to stop and show summary of pages scanned so far".yellow()
            );
        }

        let end_pfn = count.map(|c| start_pfn + c).unwrap_or(u64::MAX);

        loop {
            if pfn >= end_pfn {
                break;
            }

            // Check for interrupt signal every 1000 pages
            if total_pages % 1000 == 0 && interrupt_flag.load(Ordering::Relaxed) {
                println!(
                    "\n{}",
                    "Interrupt received! Stopping scan and showing summary..."
                        .yellow()
                        .bold()
                );
                break;
            }

            match self.read_page_flags(pfn) {
                Ok(Some(flags)) => {
                    total_pages += 1;
                    consecutive_failures = 0;

                    if flags != 0 {
                        pages_with_flags += 1;

                        // Count individual flags using array indexing (faster than HashMap)
                        for (i, (flag, _, _, category)) in PAGE_FLAGS.iter().enumerate() {
                            if flags & flag != 0 {
                                flag_counts[i] += 1;
                                category_counts[*category as usize] += 1;
                            }
                        }
                    }

                    // Show progress every 50,000 pages
                    if total_pages % 50000 == 0 {
                        let progress = if estimated_total > 0 {
                            format!(
                                " ({:.1}%)",
                                (total_pages as f64 / estimated_total as f64) * 100.0
                            )
                        } else {
                            String::new()
                        };
                        println!(
                            "Scanned {} pages so far{}",
                            total_pages.to_string().green(),
                            progress.yellow()
                        );
                    }
                }
                Ok(None) => {
                    consecutive_failures += 1;
                    if consecutive_failures > MAX_CONSECUTIVE_FAILURES {
                        break;
                    }
                }
                Err(_) => {
                    consecutive_failures += 1;
                    if consecutive_failures > MAX_CONSECUTIVE_FAILURES {
                        break;
                    }
                }
            }

            pfn += 1;

            // Safety check: don't read more than 100M pages (400GB of memory)
            if total_pages > 100_000_000 {
                println!(
                    "{}",
                    "Warning: Reached safety limit of 100M pages. Stopping.".yellow()
                );
                break;
            }
        }

        let status_msg = if interrupt_flag.load(Ordering::Relaxed) {
            format!(
                "Scan interrupted - successfully scanned {} pages",
                total_pages
            )
        } else {
            format!("Successfully scanned {} total pages", total_pages)
        };

        println!("{}", status_msg.green().bold());

        // Print optimized summary using arrays instead of HashMaps
        self.print_optimized_summary(
            total_pages,
            pages_with_flags,
            &flag_counts,
            &category_counts,
            show_histogram,
        );

        Ok(())
    }

    fn print_optimized_summary(
        &self,
        total_pages: u32,
        pages_with_flags: u32,
        flag_counts: &[u32],
        category_counts: &[u32],
        show_histogram: bool,
    ) {
        println!("\n{}", "=== SUMMARY ===".blue().bold());
        println!("Total pages analyzed: {}", total_pages.to_string().cyan());
        println!("Pages with flags: {}", pages_with_flags.to_string().green());
        println!(
            "Pages without flags: {}",
            (total_pages - pages_with_flags).to_string().yellow()
        );

        // Find flags with non-zero counts and sort them
        let mut flag_data: Vec<(usize, u32)> = flag_counts
            .iter()
            .enumerate()
            .filter(|(_, &count)| count > 0)
            .map(|(i, &count)| (i, count))
            .collect();

        if !flag_data.is_empty() {
            flag_data.sort_by(|a, b| b.1.cmp(&a.1));

            println!("\n{}", "Flag distribution:".blue().bold());
            for (flag_idx, count) in &flag_data {
                let flag_name = PAGE_FLAGS[*flag_idx].1;
                let percentage = (*count as f64 / total_pages as f64) * 100.0;
                println!(
                    "  {}: {} ({:.1}%)",
                    flag_name.green().bold(),
                    count.to_string().white(),
                    percentage.to_string().yellow()
                );
            }

            // Show histogram if requested
            if show_histogram {
                self.print_optimized_histogram(&flag_data, total_pages);
            }
        }

        // Print category summary
        self.print_optimized_category_summary(category_counts, total_pages);
    }

    fn print_optimized_histogram(&self, flag_data: &[(usize, u32)], total_pages: u32) {
        println!("\n{}", "=== HISTOGRAM ===".blue().bold());

        let max_count = flag_data.iter().map(|(_, count)| *count).max().unwrap_or(1);
        let histogram_width = 60;

        // Take top 15 flags to avoid cluttering
        let top_flags = if flag_data.len() > 15 {
            &flag_data[..15]
        } else {
            flag_data
        };

        for (flag_idx, count) in top_flags {
            let flag_name = PAGE_FLAGS[*flag_idx].1;
            let bar_length = (*count as f64 / max_count as f64 * histogram_width as f64) as usize;
            let percentage = (*count as f64 / total_pages as f64) * 100.0;

            let bar = "█".repeat(bar_length);
            println!(
                "{:>12}: {} {} ({:.1}%)",
                flag_name.green().bold(),
                bar.blue(),
                count.to_string().white(),
                percentage.to_string().yellow()
            );
        }
    }

    fn print_optimized_category_summary(&self, category_counts: &[u32], total_pages: u32) {
        // Create category data for non-zero counts
        let mut category_data: Vec<(FlagCategory, u32)> = Vec::new();

        for (i, &count) in category_counts.iter().enumerate() {
            if count > 0 {
                // Convert index back to FlagCategory enum
                let category = match i {
                    0 => FlagCategory::State,
                    1 => FlagCategory::Memory,
                    2 => FlagCategory::Usage,
                    3 => FlagCategory::Allocation,
                    4 => FlagCategory::IO,
                    5 => FlagCategory::Structure,
                    6 => FlagCategory::Special,
                    7 => FlagCategory::Error,
                    _ => continue,
                };
                category_data.push((category, count));
            }
        }

        if !category_data.is_empty() {
            category_data.sort_by(|a, b| b.1.cmp(&a.1));

            println!("\n{}", "Flag categories:".blue().bold());
            for (category, count) in category_data {
                let (symbol_char, color) = get_category_symbol_and_color(category);
                let percentage = (count as f64 / total_pages as f64) * 100.0;
                println!(
                    "  {} {:?}: {} ({:.1}%)",
                    symbol_char.to_string().color(color).bold(),
                    category,
                    count.to_string().white(),
                    percentage.to_string().yellow()
                );
            }
        }
    }

    /// Sampling mode for fast statistical overview
    /// Randomly samples pages across the entire memory space for quick analysis
    pub fn scan_sampled_summary(
        &mut self,
        sample_size: u32,
        interrupt_flag: Arc<AtomicBool>,
        show_histogram: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Pre-allocate fixed-size arrays for counters
        const MAX_FLAGS: usize = PAGE_FLAGS.len();
        let mut flag_counts = [0u32; MAX_FLAGS];
        let mut category_counts = [0u32; 8]; // 8 categories in FlagCategory enum

        let mut pages_with_flags = 0u32;
        let mut successful_reads = 0u32;

        // Estimate the maximum PFN by trying to determine system memory size
        let estimated_max_pfn = self.estimate_max_pfn()?;

        println!(
            "Sampling {} pages from estimated {} total pages for fast statistical overview...",
            sample_size.to_string().cyan(),
            estimated_max_pfn.to_string().yellow()
        );
        println!(
            "Estimated coverage: {:.3}% of total memory",
            (sample_size as f64 / estimated_max_pfn as f64 * 100.0)
                .to_string()
                .green()
        );
        println!(
            "{}",
            "Press Ctrl-C to stop and show summary of samples collected so far".yellow()
        );

        let mut rng = rand::thread_rng();
        let mut attempts = 0u32;
        let max_attempts: u32 = sample_size * 10; // Allow up to 10x attempts to handle sparse regions

        while successful_reads < sample_size && attempts < max_attempts {
            // Check for interrupt signal every 100 attempts
            if attempts % 100 == 0 && interrupt_flag.load(Ordering::Relaxed) {
                println!(
                    "\n{}",
                    "Interrupt received! Stopping sampling and showing summary..."
                        .yellow()
                        .bold()
                );
                break;
            }

            // Generate random PFN within estimated range
            let random_pfn = rng.gen_range(0..estimated_max_pfn);
            attempts += 1;

            match self.read_page_flags(random_pfn) {
                Ok(Some(flags)) => {
                    successful_reads += 1;

                    if flags != 0 {
                        pages_with_flags += 1;

                        // Count individual flags using array indexing
                        for (i, (flag, _, _, category)) in PAGE_FLAGS.iter().enumerate() {
                            if flags & flag != 0 {
                                flag_counts[i] += 1;
                                category_counts[*category as usize] += 1;
                            }
                        }
                    }

                    // Show progress every 1000 successful samples
                    if successful_reads % 1000 == 0 {
                        let progress = (successful_reads as f64 / sample_size as f64) * 100.0;
                        println!(
                            "Sampled {} pages so far ({:.1}% complete, {} attempts)",
                            successful_reads.to_string().green(),
                            progress.to_string().yellow(),
                            attempts.to_string().dimmed()
                        );
                    }
                }
                Ok(None) => {
                    // Page doesn't exist, continue sampling
                    continue;
                }
                Err(_) => {
                    // Error reading page, continue sampling
                    continue;
                }
            }
        }

        let status_msg = if interrupt_flag.load(Ordering::Relaxed) {
            format!(
                "Sampling interrupted - collected {} samples from {} attempts",
                successful_reads, attempts
            )
        } else if successful_reads < sample_size {
            format!("Sampling completed - collected {} samples from {} attempts (some regions may be sparse)", successful_reads, attempts)
        } else {
            format!(
                "Sampling completed successfully - {} samples from {} attempts",
                successful_reads, attempts
            )
        };

        println!("{}", status_msg.green().bold());

        // Calculate and display sampling statistics
        let sampling_efficiency = (successful_reads as f64 / attempts as f64) * 100.0;
        println!(
            "Sampling efficiency: {:.1}% ({} successful reads out of {} attempts)",
            sampling_efficiency.to_string().cyan(),
            successful_reads.to_string().green(),
            attempts.to_string().yellow()
        );

        // Print sampled summary with extrapolation
        self.print_sampled_summary(
            successful_reads,
            pages_with_flags,
            &flag_counts,
            &category_counts,
            estimated_max_pfn,
            show_histogram,
        );

        Ok(())
    }

    /// Estimate maximum PFN by checking system memory
    fn estimate_max_pfn(&self) -> Result<u64, Box<dyn std::error::Error>> {
        // Try to get total memory from /proc/meminfo
        match get_estimated_total_pages() {
            Ok(pages) => Ok(pages),
            Err(_) => {
                // Fallback: try to find the actual end by binary search
                // This is more expensive but more accurate
                println!("Estimating memory size by probing...");
                Ok(self.binary_search_max_pfn()?)
            }
        }
    }

    /// Binary search to find the approximate maximum valid PFN
    fn binary_search_max_pfn(&self) -> Result<u64, Box<dyn std::error::Error>> {
        let mut low = 0u64;
        let mut high = 100_000_000u64; // Start with 400GB assumption
        let mut last_valid = 0u64;

        // First, find an upper bound where reads consistently fail
        while high - low > 1000 {
            let mid = (low + high) / 2;

            // Test a few pages around the midpoint
            let mut valid_count = 0;
            for offset in 0..10 {
                if let Ok(Some(_)) = self.read_page_flags_const(mid + offset) {
                    valid_count += 1;
                    last_valid = mid + offset;
                }
            }

            if valid_count > 0 {
                low = mid;
            } else {
                high = mid;
            }
        }

        // Add some buffer for sparse regions
        Ok((last_valid + 10000).max(1_000_000)) // At least 1M pages
    }

    /// Read page flags without mutable self (for binary search)
    fn read_page_flags_const(&self, pfn: u64) -> Result<Option<u64>, Box<dyn std::error::Error>> {
        let mut file = File::open("/proc/kpageflags")?;
        let offset = pfn * 8;
        file.seek(SeekFrom::Start(offset))?;

        match file.read_u64::<LittleEndian>() {
            Ok(flags) => Ok(Some(flags)),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(Box::new(e)),
        }
    }

    fn print_sampled_summary(
        &self,
        samples_collected: u32,
        pages_with_flags: u32,
        flag_counts: &[u32],
        category_counts: &[u32],
        estimated_total_pages: u64,
        show_histogram: bool,
    ) {
        println!("\n{}", "=== SAMPLED SUMMARY ===".blue().bold());
        println!(
            "Samples collected: {}",
            samples_collected.to_string().cyan()
        );
        println!(
            "Estimated total pages in system: {}",
            estimated_total_pages.to_string().yellow()
        );
        println!(
            "Sampling coverage: {:.3}%",
            (samples_collected as f64 / estimated_total_pages as f64 * 100.0)
                .to_string()
                .green()
        );

        println!("\n{}", "Sample Statistics:".blue().bold());
        println!(
            "Pages with flags: {} ({:.1}%)",
            pages_with_flags.to_string().green(),
            (pages_with_flags as f64 / samples_collected as f64 * 100.0)
                .to_string()
                .yellow()
        );
        println!(
            "Pages without flags: {} ({:.1}%)",
            (samples_collected - pages_with_flags).to_string().yellow(),
            ((samples_collected - pages_with_flags) as f64 / samples_collected as f64 * 100.0)
                .to_string()
                .yellow()
        );

        // Extrapolate to full system
        let extrapolation_factor = estimated_total_pages as f64 / samples_collected as f64;
        println!("\n{}", "Extrapolated System Statistics:".blue().bold());
        println!(
            "Estimated pages with flags: {} ({:.1}%)",
            ((pages_with_flags as f64 * extrapolation_factor) as u64)
                .to_string()
                .green(),
            (pages_with_flags as f64 / samples_collected as f64 * 100.0)
                .to_string()
                .yellow()
        );

        // Find flags with non-zero counts and sort them
        let mut flag_data: Vec<(usize, u32)> = flag_counts
            .iter()
            .enumerate()
            .filter(|(_, &count)| count > 0)
            .map(|(i, &count)| (i, count))
            .collect();

        if !flag_data.is_empty() {
            flag_data.sort_by(|a, b| b.1.cmp(&a.1));

            println!("\n{}", "Flag distribution (sampled):".blue().bold());
            for (flag_idx, count) in &flag_data {
                let flag_name = PAGE_FLAGS[*flag_idx].1;
                let sample_percentage = (*count as f64 / samples_collected as f64) * 100.0;
                let estimated_total = (*count as f64 * extrapolation_factor) as u64;

                println!(
                    "  {}: {} ({:.1}% of samples, ~{} estimated total)",
                    flag_name.green().bold(),
                    count.to_string().white(),
                    sample_percentage.to_string().yellow(),
                    estimated_total.to_string().cyan()
                );
            }

            // Show histogram if requested
            if show_histogram {
                self.print_sampled_histogram(&flag_data, samples_collected, extrapolation_factor);
            }
        }

        // Print category summary
        self.print_sampled_category_summary(
            category_counts,
            samples_collected,
            extrapolation_factor,
        );
    }

    fn print_sampled_histogram(
        &self,
        flag_data: &[(usize, u32)],
        samples_collected: u32,
        extrapolation_factor: f64,
    ) {
        println!("\n{}", "=== SAMPLED HISTOGRAM ===".blue().bold());

        let max_count = flag_data.iter().map(|(_, count)| *count).max().unwrap_or(1);
        let histogram_width = 60;

        // Take top 15 flags to avoid cluttering
        let top_flags = if flag_data.len() > 15 {
            &flag_data[..15]
        } else {
            flag_data
        };

        for (flag_idx, count) in top_flags {
            let flag_name = PAGE_FLAGS[*flag_idx].1;
            let bar_length = (*count as f64 / max_count as f64 * histogram_width as f64) as usize;
            let sample_percentage = (*count as f64 / samples_collected as f64) * 100.0;
            let estimated_total = (*count as f64 * extrapolation_factor) as u64;

            let bar = "█".repeat(bar_length);
            println!(
                "{:>12}: {} {} ({:.1}%, ~{})",
                flag_name.green().bold(),
                bar.blue(),
                count.to_string().white(),
                sample_percentage.to_string().yellow(),
                estimated_total.to_string().cyan()
            );
        }
    }

    fn print_sampled_category_summary(
        &self,
        category_counts: &[u32],
        samples_collected: u32,
        extrapolation_factor: f64,
    ) {
        // Create category data for non-zero counts
        let mut category_data: Vec<(FlagCategory, u32)> = Vec::new();

        for (i, &count) in category_counts.iter().enumerate() {
            if count > 0 {
                let category = match i {
                    0 => FlagCategory::State,
                    1 => FlagCategory::Memory,
                    2 => FlagCategory::Usage,
                    3 => FlagCategory::Allocation,
                    4 => FlagCategory::IO,
                    5 => FlagCategory::Structure,
                    6 => FlagCategory::Special,
                    7 => FlagCategory::Error,
                    _ => continue,
                };
                category_data.push((category, count));
            }
        }

        if !category_data.is_empty() {
            category_data.sort_by(|a, b| b.1.cmp(&a.1));

            println!("\n{}", "Flag categories (sampled):".blue().bold());
            for (category, count) in category_data {
                let (symbol_char, color) = get_category_symbol_and_color(category);
                let sample_percentage = (count as f64 / samples_collected as f64) * 100.0;
                let estimated_total = (count as f64 * extrapolation_factor) as u64;

                println!(
                    "  {} {:?}: {} ({:.1}% of samples, ~{} estimated total)",
                    symbol_char.to_string().color(color).bold(),
                    category,
                    count.to_string().white(),
                    sample_percentage.to_string().yellow(),
                    estimated_total.to_string().cyan()
                );
            }
        }
    }
}

fn print_page_info(page: &PageInfo, verbose: bool) {
    let pfn_str = format!("PFN: 0x{:x}", page.pfn);
    let flags_str = format!("Flags: 0x{:016x}", page.flags);

    println!("{} {}", pfn_str.cyan().bold(), flags_str.yellow());

    if page.flags == 0 {
        println!("  {}", "No flags set".dimmed());
        return;
    }

    let flag_info = page.get_flag_descriptions();
    let unknown_flags = page.get_unknown_flags();

    if verbose {
        for (name, desc) in flag_info {
            println!("  {} - {}", name.green().bold(), desc.white());
        }
        if !unknown_flags.is_empty() {
            for bit in unknown_flags {
                println!(
                    "  {} - {}",
                    format!("UNKNOWN_BIT_{}", bit).red().bold(),
                    "Unknown flag bit".dimmed()
                );
            }
        }
    } else {
        let mut all_flags = page.get_flag_names();
        for bit in unknown_flags {
            all_flags.push(Box::leak(format!("UNKNOWN_BIT_{}", bit).into_boxed_str()));
        }
        if !all_flags.is_empty() {
            let known_flags = all_flags
                .iter()
                .filter(|f| !f.starts_with("UNKNOWN"))
                .map(|s| s.green())
                .collect::<Vec<_>>();
            let unknown_flags_colored = all_flags
                .iter()
                .filter(|f| f.starts_with("UNKNOWN"))
                .map(|s| s.red())
                .collect::<Vec<_>>();

            let mut display_flags = Vec::new();
            display_flags.extend(known_flags.iter().map(|f| f.to_string()));
            display_flags.extend(unknown_flags_colored.iter().map(|f| f.to_string()));

            println!("  {}", display_flags.join(", "));
        }
    }
}

fn print_summary(pages: &[PageInfo], show_histogram: bool) {
    let mut flag_counts: HashMap<&str, u32> = HashMap::new();
    let mut total_pages = 0;
    let mut pages_with_flags = 0;

    for page in pages {
        total_pages += 1;
        if page.flags != 0 {
            pages_with_flags += 1;
            for (flag, name, _, _) in PAGE_FLAGS {
                if page.flags & flag != 0 {
                    *flag_counts.entry(name).or_insert(0) += 1;
                }
            }
        }
    }

    println!("\n{}", "=== SUMMARY ===".blue().bold());
    println!("Total pages analyzed: {}", total_pages.to_string().cyan());
    println!("Pages with flags: {}", pages_with_flags.to_string().green());
    println!(
        "Pages without flags: {}",
        (total_pages - pages_with_flags).to_string().yellow()
    );

    if !flag_counts.is_empty() {
        println!("\n{}", "Flag distribution:".blue().bold());
        let mut sorted_flags: Vec<_> = flag_counts.iter().collect();
        sorted_flags.sort_by(|a, b| b.1.cmp(a.1));

        for (flag, count) in sorted_flags.iter() {
            let percentage = (**count as f64 / total_pages as f64) * 100.0;
            println!(
                "  {}: {} ({:.1}%)",
                flag.green().bold(),
                count.to_string().white(),
                percentage.to_string().yellow()
            );
        }

        // Show histogram if requested
        if show_histogram {
            let histogram_data: Vec<(&str, u32)> = sorted_flags
                .iter()
                .map(|(name, count)| (**name, **count))
                .collect();
            print_histogram(&histogram_data, total_pages);
        }
    }

    // Add category summary
    print_category_summary(pages);
}

fn print_histogram(sorted_flags: &[(&str, u32)], total_pages: u32) {
    println!("\n{}", "=== HISTOGRAM ===".blue().bold());

    // Calculate the maximum count for scaling
    let max_count = sorted_flags
        .iter()
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(1);
    let histogram_width = 60; // Width of the histogram bars

    // Take top 15 flags to avoid cluttering
    let top_flags = if sorted_flags.len() > 15 {
        &sorted_flags[..15]
    } else {
        sorted_flags
    };

    for (flag, count) in top_flags {
        let count_val = *count;
        let percentage = (count_val as f64 / total_pages as f64) * 100.0;

        // Calculate bar length (minimum 1 if count > 0)
        let bar_length = if count_val == 0 {
            0
        } else {
            std::cmp::max(
                1,
                (count_val as f64 / max_count as f64 * histogram_width as f64) as usize,
            )
        };

        // Create the bar with different colors based on flag category
        let bar_char = get_flag_category_char(flag);
        let bar_color = get_flag_category_color(flag);
        let bar = bar_char.repeat(bar_length).color(bar_color);

        // Format the line
        println!(
            "{:>12} │{:<60} │ {} ({:.1}%)",
            flag.green().bold(),
            bar,
            count_val.to_string().white(),
            percentage.to_string().yellow()
        );
    }

    if sorted_flags.len() > 15 {
        println!(
            "  {} (showing top 15 of {} flags)",
            "...".dimmed(),
            sorted_flags.len()
        );
    }

    // Add a scale reference
    println!("\n{}", "Scale:".dimmed());
    println!(
        "  {} = {} pages",
        "█".repeat(10).white(),
        (max_count / 6).to_string().dimmed()
    );
}

fn get_flag_category_char(flag_name: &str) -> &'static str {
    // Find the flag category and return appropriate character
    for (_, name, _, category) in PAGE_FLAGS {
        if *name == flag_name {
            return match category {
                FlagCategory::State => "█",      // Solid block
                FlagCategory::Memory => "▓",     // Dark shade
                FlagCategory::Usage => "▒",      // Medium shade
                FlagCategory::Allocation => "░", // Light shade
                FlagCategory::IO => "▄",         // Lower half block
                FlagCategory::Structure => "▀",  // Upper half block
                FlagCategory::Special => "■",    // Small solid square
                FlagCategory::Error => "▬",      // Horizontal bar
            };
        }
    }
    "█" // Default
}

fn get_flag_category_color(flag_name: &str) -> colored::Color {
    // Find the flag category and return appropriate color
    for (_, name, _, category) in PAGE_FLAGS {
        if *name == flag_name {
            let (_, color) = get_category_symbol_and_color(*category);
            return color;
        }
    }
    colored::Color::White // Default
}

pub fn get_category_symbol_and_color(category: FlagCategory) -> (char, colored::Color) {
    match category {
        FlagCategory::State => ('S', colored::Color::Blue),
        FlagCategory::Memory => ('M', colored::Color::Green),
        FlagCategory::Usage => ('U', colored::Color::Yellow),
        FlagCategory::Allocation => ('A', colored::Color::Cyan),
        FlagCategory::IO => ('I', colored::Color::Magenta),
        FlagCategory::Structure => ('T', colored::Color::Red),
        FlagCategory::Special => ('P', colored::Color::White),
        FlagCategory::Error => ('E', colored::Color::BrightRed),
    }
}

fn visualize_flags_grid(pages: &[PageInfo], width: usize) {
    println!("\n{}", "=== FLAG VISUALIZATION ===".blue().bold());

    // Print legend
    println!("{}", "Legend:".bold());
    println!("  {} = no flags", ".".dimmed());
    println!(
        "  {} = State flags (LOCKED, DIRTY, etc.)",
        "S".color(colored::Color::Blue)
    );
    println!(
        "  {} = Memory mgmt (LRU, ACTIVE, etc.)",
        "M".color(colored::Color::Green)
    );
    println!(
        "  {} = Usage tracking (REFERENCED, ANON, etc.)",
        "U".color(colored::Color::Yellow)
    );
    println!(
        "  {} = Allocation (BUDDY, SLAB)",
        "A".color(colored::Color::Cyan)
    );
    println!(
        "  {} = I/O related (WRITEBACK)",
        "I".color(colored::Color::Magenta)
    );
    println!(
        "  {} = Structure (HUGE, THP, etc.)",
        "T".color(colored::Color::Red)
    );
    println!(
        "  {} = Special (KSM, ZERO_PAGE, etc.)",
        "P".color(colored::Color::White)
    );
    println!(
        "  {} = Error flags (ERROR, HWPOISON)",
        "E".color(colored::Color::BrightRed)
    );
    println!("  {} = Multiple categories", "●".bright_white().bold());
    println!();

    for (i, page) in pages.iter().enumerate() {
        if i % width == 0 && i > 0 {
            println!();
        }

        let symbol = if page.flags == 0 {
            ".".dimmed()
        } else {
            let categories = page.get_flag_categories();
            if categories.len() == 1 {
                let (symbol_char, color) = get_category_symbol_and_color(categories[0]);
                symbol_char.to_string().color(color)
            } else if categories.len() > 1 {
                "●".bright_white().bold()
            } else {
                "?".red() // Unknown flags
            }
        };

        print!("{}", symbol);
    }
    println!();
}

fn print_category_summary(pages: &[PageInfo]) {
    let mut category_counts: HashMap<FlagCategory, u32> = HashMap::new();

    for page in pages {
        for category in page.get_flag_categories() {
            *category_counts.entry(category).or_insert(0) += 1;
        }
    }

    if !category_counts.is_empty() {
        println!("\n{}", "Flag categories:".blue().bold());
        let mut sorted_categories: Vec<_> = category_counts.iter().collect();
        sorted_categories.sort_by(|a, b| b.1.cmp(a.1));

        for (category, count) in sorted_categories {
            let (symbol_char, color) = get_category_symbol_and_color(*category);
            let percentage = (*count as f64 / pages.len() as f64) * 100.0;
            println!(
                "  {} {:?}: {} ({:.1}%)",
                symbol_char.to_string().color(color).bold(),
                category,
                count.to_string().white(),
                percentage.to_string().yellow()
            );
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up Ctrl-C handler
    let interrupt_flag = Arc::new(AtomicBool::new(false));
    let interrupt_flag_clone = interrupt_flag.clone();

    ctrlc::set_handler(move || {
        interrupt_flag_clone.store(true, Ordering::Relaxed);
    })?;
    let matches = Command::new("kpageflags-visualizer")
        .about("Visualize Linux kernel page flags from /proc/kpageflags")
        .arg(
            Arg::new("start")
                .short('s')
                .long("start")
                .value_name("PFN")
                .help("Starting page frame number (hex or decimal)")
                .default_value("0"),
        )
        .arg(
            Arg::new("count")
                .short('c')
                .long("count")
                .value_name("COUNT")
                .help("Number of pages to analyze (use 'all' for all available pages)")
                .default_value("all"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Show detailed flag descriptions")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("summary")
                .long("summary")
                .help("Show only summary statistics")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("sampled")
                .long("sampled")
                .value_name("SAMPLES")
                .help("Use sampling mode for fast statistical overview (default: 10000 samples)")
                .default_missing_value("10000")
                .num_args(0..=1),
        )
        .arg(
            Arg::new("grid")
                .short('g')
                .long("grid")
                .help("Show grid visualization")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("limit")
                .short('l')
                .long("limit")
                .value_name("LIMIT")
                .help("Limit individual page output (use with large datasets)")
                .default_value("1000"),
        )
        .arg(
            Arg::new("width")
                .short('w')
                .long("width")
                .value_name("WIDTH")
                .help("Grid width for visualization")
                .default_value("80"),
        )
        .arg(
            Arg::new("histogram")
                .long("histogram")
                .help("Show histogram visualization in summary")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("tui")
                .long("tui")
                .help("Launch interactive TUI mode")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    // Parse arguments
    let start_pfn = if let Some(start_str) = matches.get_one::<String>("start") {
        if start_str.starts_with("0x") {
            u64::from_str_radix(&start_str[2..], 16)?
        } else {
            start_str.parse::<u64>()?
        }
    } else {
        0
    };

    let count: u64 = if let Some(count_str) = matches.get_one::<String>("count") {
        if count_str == "all" {
            u64::MAX // Special value indicating "read all available pages"
        } else {
            count_str.parse()?
        }
    } else {
        u64::MAX // Default to all pages
    };

    let verbose = matches.get_flag("verbose");
    let summary_only = matches.get_flag("summary");
    let sampled_mode = matches.get_one::<String>("sampled");
    let show_grid = matches.get_flag("grid");
    let show_histogram = matches.get_flag("histogram");
    let tui_mode = matches.get_flag("tui");
    let grid_width: usize = matches.get_one::<String>("width").unwrap().parse()?;
    let output_limit: usize = matches.get_one::<String>("limit").unwrap().parse()?;

    // Check if we have permission to read kpageflags
    if !std::path::Path::new("/proc/kpageflags").exists() {
        eprintln!(
            "{}",
            "Error: /proc/kpageflags not found. Make sure you're running on Linux.".red()
        );
        return Ok(());
    }

    // Launch TUI mode if requested
    if tui_mode {
        println!("{}", "Launching KPageFlags TUI...".green().bold());
        return tui::run_tui().await;
    }

    println!("{}", "KPageFlags Visualizer".blue().bold());

    let mut reader = KPageFlagsReader::new()?;

    // Use sampling mode if --sampled flag is set
    if let Some(sample_str) = sampled_mode {
        let sample_size: u32 = sample_str.parse().unwrap_or(10000);
        println!(
            "{}",
            "Using sampling mode for fast statistical overview".green()
        );
        println!("Sample size: {} pages", sample_size.to_string().cyan());
        println!("{}", "=".repeat(50).blue());

        reader.scan_sampled_summary(sample_size, interrupt_flag.clone(), show_histogram)?;
        return Ok(());
    }

    // Use optimized summary-only scanning if --summary flag is set
    if summary_only {
        println!(
            "{}",
            "Using optimized summary mode (minimal memory usage)".green()
        );

        if count == u64::MAX {
            println!(
                "Analyzing ALL available pages starting from PFN 0x{:x} (summary only)",
                start_pfn
            );
            println!("{}", "=".repeat(50).blue());
            reader.scan_for_summary_only(
                start_pfn,
                None,
                interrupt_flag.clone(),
                show_histogram,
            )?;
        } else {
            println!(
                "Analyzing {} pages starting from PFN 0x{:x} (summary only)",
                count, start_pfn
            );
            println!("{}", "=".repeat(50).blue());
            reader.scan_for_summary_only(
                start_pfn,
                Some(count),
                interrupt_flag.clone(),
                show_histogram,
            )?;
        }

        // Early return - no need to process individual pages
        return Ok(());
    }

    let pages = if count == u64::MAX {
        println!(
            "Analyzing ALL available pages starting from PFN 0x{:x}",
            start_pfn
        );
        println!("{}", "=".repeat(50).blue());
        reader.read_all_pages(start_pfn, interrupt_flag.clone())?
    } else {
        println!(
            "Analyzing {} pages starting from PFN 0x{:x}",
            count, start_pfn
        );
        if count > output_limit as u64 && !summary_only {
            println!(
                "{}",
                format!(
                    "Note: Individual page output limited to first {} pages",
                    output_limit
                )
                .yellow()
            );
        }
        println!("{}", "=".repeat(50).blue());

        // Show progress for large datasets
        if count > 10000 {
            println!(
                "{}",
                "Reading page flags... (this may take a moment for large datasets)".yellow()
            );
            println!(
                "{}",
                "Press Ctrl-C to stop and show summary of pages scanned so far".yellow()
            );
        }

        reader.read_range(start_pfn, count, interrupt_flag.clone())?
    };

    if pages.is_empty() {
        println!("{}", "No pages found in the specified range.".yellow());
        return Ok(());
    }

    if !summary_only {
        // Print individual page information (limited)
        let pages_to_show = if pages.len() > output_limit {
            if count == u64::MAX {
                println!(
                    "{}",
                    format!(
                        "Note: Individual page output limited to first {} of {} total pages",
                        output_limit,
                        pages.len()
                    )
                    .yellow()
                );
            }
            println!(
                "{}",
                format!("Showing first {} of {} pages:", output_limit, pages.len()).yellow()
            );
            &pages[..output_limit]
        } else {
            &pages
        };

        for page in pages_to_show {
            print_page_info(page, verbose);
            println!();
        }

        if pages.len() > output_limit {
            println!(
                "{}",
                format!(
                    "... and {} more pages (use --summary to see all statistics)",
                    pages.len() - output_limit
                )
                .dimmed()
            );
        }
    }

    // Always show summary
    print_summary(&pages, show_histogram);

    // Show grid visualization if requested
    if show_grid {
        visualize_flags_grid(&pages, grid_width);
    }

    Ok(())
}
