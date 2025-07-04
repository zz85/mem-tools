# KPageFlags Visualizer

A Rust program to visualize Linux kernel page flags from `/proc/kpageflags`.

## Features

- Read and parse `/proc/kpageflags` data
- **Analyze all available pages by default**
- Display page frame numbers (PFNs) and their associated flags
- **Enhanced colorized visualization with flag categories**
- Summary statistics showing flag distribution
- **Category-based grid visualization** showing different flag types
- Support for hex and decimal PFN input
- Verbose mode with detailed flag descriptions
- **Progress indication for large datasets**
- **Output limiting for manageable display**

## Requirements

- Linux system with `/proc/kpageflags` available
- Root privileges may be required to read `/proc/kpageflags`
- Rust toolchain (cargo)

## Installation

```bash
cargo build --release
```

## Usage

### Basic usage
```bash
# Analyze ALL available pages (default)
cargo run

# Analyze first 100 pages
cargo run -- --count 100

# Analyze 50 pages starting from PFN 0x1000
cargo run -- --start 0x1000 --count 50

# Show verbose descriptions (limited output for readability)
cargo run -- --verbose

# Show only summary for all pages
cargo run -- --summary

# Show enhanced grid visualization with flag categories
cargo run -- --grid --width 60
```

### Command line options

- `-s, --start <PFN>`: Starting page frame number (hex with 0x prefix or decimal)
- `-c, --count <COUNT>`: Number of pages to analyze (use 'all' for all available pages, default: 'all')
- `-v, --verbose`: Show detailed flag descriptions
- `--summary`: Show only summary statistics
- `-g, --grid`: Show enhanced grid visualization with flag categories
- `-w, --width <WIDTH>`: Grid width for visualization (default: 80)
- `-l, --limit <LIMIT>`: Limit individual page output for large datasets (default: 1000)
- `--histogram`: Show histogram visualization in summary

### Examples

```bash
# Analyze all pages with summary and grid
cargo run -- --summary --grid

# Analyze memory around a specific address
cargo run -- -s 0x10000 -c 200 --grid

# Get detailed information about first 10 pages
cargo run -- -c 10 --verbose

# Quick overview of all available pages
cargo run -- --summary --grid --width 120

# Show histogram of flag distribution for fast analysis
cargo run -- --summary --histogram

# Combine histogram with grid for comprehensive visualization
cargo run -- --summary --histogram --grid
```

## Enhanced Visualization

The program now provides **category-based visualization** with different symbols and colors for different flag types:

### Flag Categories

- **S** (Blue) - **State flags**: LOCKED, DIRTY, UPTODATE, etc.
- **M** (Green) - **Memory management**: LRU, ACTIVE, RECLAIM, etc.
- **U** (Yellow) - **Usage tracking**: REFERENCED, ANON, IDLE, etc.
- **A** (Cyan) - **Allocation type**: BUDDY, SLAB
- **I** (Magenta) - **I/O related**: WRITEBACK
- **T** (Red) - **Structure**: HUGE, THP, COMPOUND_HEAD/TAIL
- **P** (White) - **Special purpose**: KSM, ZERO_PAGE, PGTABLE
- **E** (Bright Red) - **Error flags**: ERROR, HWPOISON
- **●** (Bright White) - **Multiple categories**
- **.** (Dimmed) - **No flags**

## Page Flags

The program recognizes the following page flags:

- **LOCKED**: Page is locked
- **ERROR**: Page has error
- **REFERENCED**: Page has been referenced
- **UPTODATE**: Page is up to date
- **DIRTY**: Page is dirty
- **LRU**: Page is on LRU list
- **ACTIVE**: Page is on active list
- **SLAB**: Page is slab allocated
- **WRITEBACK**: Page is under writeback
- **RECLAIM**: Page is being reclaimed
- **BUDDY**: Page is free buddy page
- **MMAP**: Page is memory mapped
- **ANON**: Page is anonymous
- **SWAPCACHE**: Page is in swap cache
- **SWAPBACKED**: Page is swap backed
- **COMPOUND_HEAD**: Compound page head
- **COMPOUND_TAIL**: Compound page tail
- **HUGE**: Huge page
- **UNEVICTABLE**: Page is unevictable
- **HWPOISON**: Hardware poisoned page
- **NOPAGE**: No page frame exists
- **KSM**: KSM page
- **THP**: Transparent huge page
- **OFFLINE**: Page is offline
- **ZERO_PAGE**: Zero page
- **IDLE**: Page is idle
- **PGTABLE**: Page table page
- **RESERVED**: Reserved page (common in early memory)

## Output Format

### Individual Page Information
```
PFN: 0x1234 Flags: 0x0000000000000020
  LRU

PFN: 0x1235 Flags: 0x0000000000000068
  UPTODATE, LRU, ACTIVE
```

### Enhanced Summary Statistics
```
=== SUMMARY ===
Total pages analyzed: 1048576
Pages with flags: 524288
Pages without flags: 524288

Flag distribution:
  BUDDY: 300000 (28.6%)
  RESERVED: 100000 (9.5%)
  LRU: 50000 (4.8%)

Flag categories:
  A Allocation: 300000 (28.6%)
  S State: 150000 (14.3%)
  M Memory: 74288 (7.1%)
```

### Enhanced Grid Visualization
```
=== FLAG VISUALIZATION ===
Legend:
  . = no flags
  S = State flags (LOCKED, DIRTY, etc.)
  M = Memory mgmt (LRU, ACTIVE, etc.)
  U = Usage tracking (REFERENCED, ANON, etc.)
  A = Allocation (BUDDY, SLAB)
  I = I/O related (WRITEBACK)
  T = Structure (HUGE, THP, etc.)
  P = Special (KSM, ZERO_PAGE, etc.)
  E = Error flags (ERROR, HWPOISON)
  ● = Multiple categories

SSSSSSSSSSSSSSSSAAAAAAAAAAAAAAAAAAAAMMMMMMMMMMUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUUU
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
```

## Permissions

You may need to run with elevated privileges:

```bash
sudo cargo run -- --summary --grid
```

## Performance Notes

- **Memory-mapped I/O**: The program automatically attempts to use memory-mapped I/O for faster access to `/proc/kpageflags`. If mmap fails (common with `/proc` files), it gracefully falls back to regular I/O
- **Large datasets**: When analyzing all pages (potentially millions), the program automatically limits individual page output to 1000 entries by default
- **Progress indication**: Shows progress for datasets larger than 10,000 pages
- **Memory efficient**: Processes pages in chunks to handle large memory systems
- **Interrupt handling**: Press Ctrl-C to stop scanning and show summary of pages read so far
- Use `--summary` flag for fastest analysis of large datasets
- **Expected performance**: 
  - With mmap: ~30-60 seconds for 1TB RAM systems (when supported)
  - Without mmap: ~2-5 minutes for 1TB RAM systems
  - Actual performance may be faster on modern systems

## Notes

- Each entry in `/proc/kpageflags` is 8 bytes (64-bit flags)
- PFN (Page Frame Number) represents physical memory pages
- Not all PFNs may have corresponding entries in kpageflags
- The program handles missing entries gracefully
- **Default behavior now analyzes ALL available pages** for comprehensive system overview
