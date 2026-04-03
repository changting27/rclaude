//! Formatting utilities for terminal output.

/// Format file size in human-readable form.
pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KB", bytes as f64 / 1024.0);
    }
    if bytes < 1024 * 1024 * 1024 {
        return format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0));
    }
    format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

/// Format duration in human-readable form.
pub fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        return format!("{ms}ms");
    }
    let secs = ms / 1000;
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = secs / 60;
    let remaining_secs = secs % 60;
    if mins < 60 {
        return format!("{mins}m {remaining_secs}s");
    }
    let hours = mins / 60;
    let remaining_mins = mins % 60;
    format!("{hours}h {remaining_mins}m")
}

/// Format number with K/M suffixes.
pub fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Format token count.
pub fn format_tokens(count: u64) -> String {
    format!("{} tokens", format_number(count))
}

/// Format cost in USD.
pub fn format_cost(usd: f64) -> String {
    if usd < 0.01 {
        format!("${:.4}", usd)
    } else if usd < 1.0 {
        format!("${:.3}", usd)
    } else {
        format!("${:.2}", usd)
    }
}

/// Format relative time (e.g., "2 minutes ago").
pub fn format_relative_time(secs_ago: u64) -> String {
    if secs_ago < 60 {
        return "just now".into();
    }
    if secs_ago < 3600 {
        return format!("{} minutes ago", secs_ago / 60);
    }
    if secs_ago < 86400 {
        return format!("{} hours ago", secs_ago / 3600);
    }
    format!("{} days ago", secs_ago / 86400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1_500_000), "1.4 MB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(5000), "5s");
        assert_eq!(format_duration(90000), "1m 30s");
        assert_eq!(format_duration(3700000), "1h 1m");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(500), "500");
        assert_eq!(format_number(1500), "1.5K");
        assert_eq!(format_number(1_500_000), "1.5M");
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.001), "$0.0010");
        assert_eq!(format_cost(0.05), "$0.050");
        assert_eq!(format_cost(1.5), "$1.50");
    }

    #[test]
    fn test_format_relative_time() {
        assert_eq!(format_relative_time(30), "just now");
        assert_eq!(format_relative_time(120), "2 minutes ago");
        assert_eq!(format_relative_time(7200), "2 hours ago");
    }
}
