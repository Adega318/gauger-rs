const SHORT_UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];
const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];

/// Compact byte size for the table, for example `1.2M` or `340K`.
pub(crate) fn bytes_compact(bytes: u64) -> String {
    let (value, unit) = scale(bytes);

    if unit == 0 {
        return format!("{bytes}B");
    }

    if value >= 10.0 {
        format!("{value:.0}{}", SHORT_UNITS[unit])
    } else {
        format!("{value:.1}{}", SHORT_UNITS[unit])
    }
}

/// Human-readable byte size, for example `1.2 MiB`.
pub(crate) fn bytes(bytes: u64) -> String {
    let (value, unit) = scale(bytes);

    if unit == 0 {
        return format!("{bytes} B");
    }

    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// A per-second rate, or a dash while the first sample is pending.
pub(crate) fn rate(rate: Option<u64>) -> String {
    match rate {
        Some(rate) => format!("{}/s", bytes(rate)),
        None => "—".to_owned(),
    }
}

pub(crate) fn percent(percent: Option<f32>) -> String {
    match percent {
        Some(percent) => format!("{percent:.1}%"),
        None => "—".to_owned(),
    }
}

fn scale(bytes: u64) -> (f64, usize) {
    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }

    (value, unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_compact_bytes() {
        assert_eq!(bytes_compact(0), "0B");
        assert_eq!(bytes_compact(999), "999B");
        assert_eq!(bytes_compact(1024), "1.0K");
        assert_eq!(bytes_compact(12 * 1024 * 1024), "12M");
    }

    #[test]
    fn formats_human_bytes() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1536), "1.5 KiB");
        assert_eq!(bytes(128 * 1024 * 1024), "128 MiB");
    }

    #[test]
    fn formats_rates_and_percent() {
        assert_eq!(rate(None), "—");
        assert_eq!(rate(Some(1024)), "1.0 KiB/s");
        assert_eq!(percent(None), "—");
        assert_eq!(percent(Some(12.34)), "12.3%");
    }
}
