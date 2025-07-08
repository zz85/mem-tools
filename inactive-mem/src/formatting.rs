/// Formatting utilities for displaying memory values with better readability

/// Format a number with comma separators (e.g., 1234567 -> "1,234,567")
pub fn format_number(n: u64) -> String {
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

/// Format a signed number with comma separators and sign (e.g., -1234567 -> "-1,234,567", 1234567 -> "+1,234,567")
pub fn format_signed_number(n: i64) -> String {
    if n >= 0 {
        format!("+{}", format_number(n as u64))
    } else {
        format!("-{}", format_number((-n) as u64))
    }
}

/// Format memory size in KB with comma separators and appropriate unit conversion
pub fn format_memory_kb(kb: u64) -> String {
    let formatted_kb = format_number(kb);

    if kb >= 1024 * 1024 * 1024 {
        // TB
        format!(
            "{} KB ({:.1} TB)",
            formatted_kb,
            kb as f64 / (1024.0 * 1024.0 * 1024.0)
        )
    } else if kb >= 1024 * 1024 {
        // GB
        format!(
            "{} KB ({:.1} GB)",
            formatted_kb,
            kb as f64 / (1024.0 * 1024.0)
        )
    } else if kb >= 1024 {
        // MB
        format!("{} KB ({:.1} MB)", formatted_kb, kb as f64 / 1024.0)
    } else {
        // Just KB
        format!("{} KB", formatted_kb)
    }
}

/// Format memory change with sign, comma separators, and appropriate unit conversion
pub fn format_memory_change_kb(kb: i64) -> String {
    let abs_kb = kb.abs() as u64;
    let sign = if kb >= 0 { "+" } else { "-" };
    let formatted_kb = format_number(abs_kb);

    if abs_kb >= 1024 * 1024 * 1024 {
        // TB
        format!(
            "{}{} KB ({}{:.1} TB)",
            sign,
            formatted_kb,
            sign,
            abs_kb as f64 / (1024.0 * 1024.0 * 1024.0)
        )
    } else if abs_kb >= 1024 * 1024 {
        // GB
        format!(
            "{}{} KB ({}{:.1} GB)",
            sign,
            formatted_kb,
            sign,
            abs_kb as f64 / (1024.0 * 1024.0)
        )
    } else if abs_kb >= 1024 {
        // MB
        format!(
            "{}{} KB ({}{:.1} MB)",
            sign,
            formatted_kb,
            sign,
            abs_kb as f64 / 1024.0
        )
    } else {
        // Just KB
        format!("{}{} KB", sign, formatted_kb)
    }
}

/// Format percentage with appropriate precision
pub fn format_percentage(ratio: f64) -> String {
    if ratio < 0.01 {
        format!("{:.3}%", ratio * 100.0)
    } else if ratio < 0.1 {
        format!("{:.2}%", ratio * 100.0)
    } else {
        format!("{:.1}%", ratio * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(123), "123");
        assert_eq!(format_number(1234), "1,234");
        assert_eq!(format_number(1234567), "1,234,567");
        assert_eq!(format_number(1234567890), "1,234,567,890");
    }

    #[test]
    fn test_format_signed_number() {
        assert_eq!(format_signed_number(0), "+0");
        assert_eq!(format_signed_number(1234), "+1,234");
        assert_eq!(format_signed_number(-1234), "-1,234");
        assert_eq!(format_signed_number(1234567), "+1,234,567");
        assert_eq!(format_signed_number(-1234567), "-1,234,567");
    }

    #[test]
    fn test_format_memory_kb() {
        assert_eq!(format_memory_kb(512), "512 KB");
        assert_eq!(format_memory_kb(1536), "1,536 KB (1.5 MB)");
        assert_eq!(format_memory_kb(2048 * 1024), "2,097,152 KB (2.0 GB)");
    }

    #[test]
    fn test_format_memory_change_kb() {
        assert_eq!(format_memory_change_kb(512), "+512 KB");
        assert_eq!(format_memory_change_kb(-512), "-512 KB");
        assert_eq!(format_memory_change_kb(1536), "+1,536 KB (+1.5 MB)");
        assert_eq!(format_memory_change_kb(-1536), "-1,536 KB (-1.5 MB)");
    }

    #[test]
    fn test_format_percentage() {
        assert_eq!(format_percentage(0.001), "0.100%");
        assert_eq!(format_percentage(0.05), "5.00%");
        assert_eq!(format_percentage(0.5), "50.0%");
        assert_eq!(format_percentage(0.999), "99.9%");
    }
}
