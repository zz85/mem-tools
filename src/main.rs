use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::collections::HashMap;
use clap::{Arg, Command};
use colored::*;
use byteorder::{LittleEndian, ReadBytesExt};

// Page flag definitions from kernel source
const PAGE_FLAGS: &[(u64, &str, &str)] = &[
    (1 << 0,  "LOCKED",      "Page is locked"),
    (1 << 1,  "ERROR",       "Page has error"),
    (1 << 2,  "REFERENCED",  "Page has been referenced"),
    (1 << 3,  "UPTODATE",    "Page is up to date"),
    (1 << 4,  "DIRTY",       "Page is dirty"),
    (1 << 5,  "LRU",         "Page is on LRU list"),
    (1 << 6,  "ACTIVE",      "Page is on active list"),
    (1 << 7,  "SLAB",        "Page is slab allocated"),
    (1 << 8,  "WRITEBACK",   "Page is under writeback"),
    (1 << 9,  "RECLAIM",     "Page is being reclaimed"),
    (1 << 10, "BUDDY",       "Page is free buddy page"),
    (1 << 11, "MMAP",        "Page is memory mapped"),
    (1 << 12, "ANON",        "Page is anonymous"),
    (1 << 13, "SWAPCACHE",   "Page is in swap cache"),
    (1 << 14, "SWAPBACKED",  "Page is swap backed"),
    (1 << 15, "COMPOUND_HEAD", "Compound page head"),
    (1 << 16, "COMPOUND_TAIL", "Compound page tail"),
    (1 << 17, "HUGE",        "Huge page"),
    (1 << 18, "UNEVICTABLE", "Page is unevictable"),
    (1 << 19, "HWPOISON",    "Hardware poisoned page"),
    (1 << 20, "NOPAGE",      "No page frame exists"),
    (1 << 21, "KSM",         "KSM page"),
    (1 << 22, "THP",         "Transparent huge page"),
    (1 << 23, "OFFLINE",     "Page is offline"),
    (1 << 24, "ZERO_PAGE",   "Zero page"),
    (1 << 25, "IDLE",        "Page is idle"),
    (1 << 26, "PGTABLE",     "Page table page"),
    // Additional flags that might be present
    (1 << 32, "RESERVED",    "Reserved page (common in early memory)"),
];

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
            .filter(|(flag, _, _)| self.flags & flag != 0)
            .map(|(_, name, _)| *name)
            .collect()
    }

    fn get_flag_descriptions(&self) -> Vec<(&'static str, &'static str)> {
        PAGE_FLAGS
            .iter()
            .filter(|(flag, _, _)| self.flags & flag != 0)
            .map(|(_, name, desc)| (*name, *desc))
            .collect()
    }

    fn get_unknown_flags(&self) -> Vec<u8> {
        let known_flags: u64 = PAGE_FLAGS.iter().map(|(flag, _, _)| flag).sum();
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

    fn read_page_flags(&mut self, pfn: u64) -> Result<Option<u64>, Box<dyn std::error::Error>> {
        let offset = pfn * 8; // Each entry is 8 bytes
        self.file.seek(SeekFrom::Start(offset))?;
        
        match self.file.read_u64::<LittleEndian>() {
            Ok(flags) => Ok(Some(flags)),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(Box::new(e)),
        }
    }

    fn read_range(&mut self, start_pfn: u64, count: u64) -> Result<Vec<PageInfo>, Box<dyn std::error::Error>> {
        let mut pages = Vec::new();
        
        for pfn in start_pfn..start_pfn + count {
            if let Some(flags) = self.read_page_flags(pfn)? {
                pages.push(PageInfo::new(pfn, flags));
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
                println!("  {} - {}", 
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
            let known_flags = all_flags.iter()
                .filter(|f| !f.starts_with("UNKNOWN"))
                .map(|s| s.green())
                .collect::<Vec<_>>();
            let unknown_flags_colored = all_flags.iter()
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
            for (flag, name, _) in PAGE_FLAGS {
                if page.flags & flag != 0 {
                    *flag_counts.entry(name).or_insert(0) += 1;
                }
            }
        }
    }

    println!("\n{}", "=== SUMMARY ===".blue().bold());
    println!("Total pages analyzed: {}", total_pages.to_string().cyan());
    println!("Pages with flags: {}", pages_with_flags.to_string().green());
    println!("Pages without flags: {}", (total_pages - pages_with_flags).to_string().yellow());

    if !flag_counts.is_empty() {
        println!("\n{}", "Flag distribution:".blue().bold());
        let mut sorted_flags: Vec<_> = flag_counts.iter().collect();
        sorted_flags.sort_by(|a, b| b.1.cmp(a.1));
        
        for (flag, count) in sorted_flags {
            let percentage = (*count as f64 / total_pages as f64) * 100.0;
            println!("  {}: {} ({:.1}%)", 
                flag.green().bold(), 
                count.to_string().white(), 
                percentage.to_string().yellow()
            );
        }
    }
}

fn visualize_flags_grid(pages: &[PageInfo], width: usize) {
    println!("\n{}", "=== FLAG VISUALIZATION ===".blue().bold());
    println!("Legend: {} = no flags, {} = has flags, {} = multiple flags", 
        ".".dimmed(), 
        "●".green(), 
        "●".red().bold()
    );
    
    for (i, page) in pages.iter().enumerate() {
        if i % width == 0 && i > 0 {
            println!();
        }
        
        let symbol = match page.flags {
            0 => ".".dimmed(),
            flags if flags.count_ones() == 1 => "●".green(),
            _ => "●".red().bold(),
        };
        
        print!("{}", symbol);
    }
    println!();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("kpageflags-visualizer")
        .about("Visualize Linux kernel page flags from /proc/kpageflags")
        .arg(Arg::new("start")
            .short('s')
            .long("start")
            .value_name("PFN")
            .help("Starting page frame number (hex or decimal)")
            .default_value("0"))
        .arg(Arg::new("count")
            .short('c')
            .long("count")
            .value_name("COUNT")
            .help("Number of pages to analyze")
            .default_value("100"))
        .arg(Arg::new("verbose")
            .short('v')
            .long("verbose")
            .help("Show detailed flag descriptions")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("summary")
            .long("summary")
            .help("Show only summary statistics")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("grid")
            .short('g')
            .long("grid")
            .help("Show grid visualization")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("width")
            .short('w')
            .long("width")
            .value_name("WIDTH")
            .help("Grid width for visualization")
            .default_value("80"))
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

    let count: u64 = matches.get_one::<String>("count")
        .unwrap()
        .parse()?;
    
    let verbose = matches.get_flag("verbose");
    let summary_only = matches.get_flag("summary");
    let show_grid = matches.get_flag("grid");
    let grid_width: usize = matches.get_one::<String>("width")
        .unwrap()
        .parse()?;

    // Check if we have permission to read kpageflags
    if !std::path::Path::new("/proc/kpageflags").exists() {
        eprintln!("{}", "Error: /proc/kpageflags not found. Make sure you're running on Linux.".red());
        return Ok(());
    }

    println!("{}", "KPageFlags Visualizer".blue().bold());
    println!("Analyzing {} pages starting from PFN 0x{:x}", count, start_pfn);
    println!("{}", "=".repeat(50).blue());

    let mut reader = KPageFlagsReader::new()?;
    let pages = reader.read_range(start_pfn, count)?;

    if pages.is_empty() {
        println!("{}", "No pages found in the specified range.".yellow());
        return Ok(());
    }

    if !summary_only {
        // Print individual page information
        for page in &pages {
            print_page_info(page, verbose);
            println!();
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
