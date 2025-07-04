use byteorder::{LittleEndian, ReadBytesExt};
use clap::{Arg, Command};
use colored::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

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
const PAGE_FLAGS: &[(u64, &str, &str, FlagCategory)] = &[
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
enum FlagCategory {
    State,      // Page state flags
    Memory,     // Memory management flags
    Usage,      // Usage tracking flags
    Allocation, // Allocation type flags
    IO,         // I/O related flags
    Structure,  // Page structure flags
    Special,    // Special purpose flags
    Error,      // Error flags
}

#[derive(Debug)]
struct PageInfo {
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

struct KPageFlagsReader {
    file: BufReader<File>,
}

impl KPageFlagsReader {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
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

        loop {
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

        println!(
            "Successfully read {} total pages",
            pages.len().to_string().green().bold()
        );
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

    fn read_range(
        &mut self,
        start_pfn: u64,
        count: u64,
    ) -> Result<Vec<PageInfo>, Box<dyn std::error::Error>> {
        let mut pages = Vec::new();
        let mut consecutive_failures = 0;
        const MAX_CONSECUTIVE_FAILURES: u32 = 1000; // Stop after 1000 consecutive failures

        for pfn in start_pfn..start_pfn + count {
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

fn print_summary(pages: &[PageInfo]) {
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

        for (flag, count) in sorted_flags {
            let percentage = (*count as f64 / total_pages as f64) * 100.0;
            println!(
                "  {}: {} ({:.1}%)",
                flag.green().bold(),
                count.to_string().white(),
                percentage.to_string().yellow()
            );
        }
    }

    // Add category summary
    print_category_summary(pages);
}

fn get_category_symbol_and_color(category: FlagCategory) -> (char, colored::Color) {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let show_grid = matches.get_flag("grid");
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

    println!("{}", "KPageFlags Visualizer".blue().bold());

    let mut reader = KPageFlagsReader::new()?;

    let pages = if count == u64::MAX {
        println!(
            "Analyzing ALL available pages starting from PFN 0x{:x}",
            start_pfn
        );
        println!("{}", "=".repeat(50).blue());
        reader.read_all_pages(start_pfn)?
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
        }

        reader.read_range(start_pfn, count)?
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
    print_summary(&pages);

    // Show grid visualization if requested
    if show_grid {
        visualize_flags_grid(&pages, grid_width);
    }

    Ok(())
}
