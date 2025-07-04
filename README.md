# KPageFlags Visualizer

A Rust program to visualize Linux kernel page flags from `/proc/kpageflags`.

## Features

- Read and parse `/proc/kpageflags` data
- Display page frame numbers (PFNs) and their associated flags
- Colorized output for better readability
- Summary statistics showing flag distribution
- Grid visualization for pattern recognition
- Support for hex and decimal PFN input
- Verbose mode with detailed flag descriptions

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
# Analyze first 100 pages
cargo run

# Analyze 50 pages starting from PFN 0x1000
cargo run -- --start 0x1000 --count 50

# Show verbose descriptions
cargo run -- --verbose

# Show only summary
cargo run -- --summary

# Show grid visualization
cargo run -- --grid --width 60
```

### Command line options

- `-s, --start <PFN>`: Starting page frame number (hex with 0x prefix or decimal)
- `-c, --count <COUNT>`: Number of pages to analyze (default: 100)
- `-v, --verbose`: Show detailed flag descriptions
- `--summary`: Show only summary statistics
- `-g, --grid`: Show grid visualization
- `-w, --width <WIDTH>`: Grid width for visualization (default: 80)

### Examples

```bash
# Analyze memory around a specific address
cargo run -- -s 0x10000 -c 200 --grid

# Get detailed information about first 10 pages
cargo run -- -c 10 --verbose

# Quick overview of a large range
cargo run -- -s 0 -c 1000 --summary --grid
```

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

## Output Format

### Individual Page Information
```
PFN: 0x1234 Flags: 0x0000000000000020
  LRU

PFN: 0x1235 Flags: 0x0000000000000068
  UPTODATE, LRU, ACTIVE
```

### Summary Statistics
```
=== SUMMARY ===
Total pages analyzed: 100
Pages with flags: 45
Pages without flags: 55

Flag distribution:
  LRU: 30 (30.0%)
  UPTODATE: 25 (25.0%)
  ACTIVE: 20 (20.0%)
```

### Grid Visualization
```
=== FLAG VISUALIZATION ===
Legend: . = no flags, ● = has flags, ● = multiple flags
..●●.●●●..●.●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●
```

## Permissions

You may need to run with elevated privileges:

```bash
sudo cargo run -- --start 0x1000 --count 100
```

## Notes

- Each entry in `/proc/kpageflags` is 8 bytes (64-bit flags)
- PFN (Page Frame Number) represents physical memory pages
- Not all PFNs may have corresponding entries in kpageflags
- The program handles missing entries gracefully
