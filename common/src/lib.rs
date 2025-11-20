use chrono::{TimeZone, NaiveDateTime, FixedOffset};

/// Quest status enum shared by both services.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestStatus {
    Upcoming,
    Ongoing,
    Ended,
    Tba,
}

/// Calculate quest status from epoch timestamps (seconds).
pub fn calculate_status(current_time: i64, start: &i64, end: &i64) -> QuestStatus {
    if *end > 0 && current_time > *end {
        QuestStatus::Ended
    } else if *start > 0 && current_time >= *start {
        QuestStatus::Ongoing
    } else if *start > 0 {
        QuestStatus::Upcoming
    } else {
        QuestStatus::Tba
    }
}

/// Parse a local WIB (Asia/Jakarta) string "YYYY-MM-DD HH:MM" into RFC3339 +07:00.
pub fn parse_wib(input: &str) -> Result<String, String> {
    let naive = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M")
        .map_err(|_| "Wrong time format! Use: YYYY-MM-DD HH:MM (E.g: 2025-11-25 19:30)".to_string())?;

    let wib_offset = FixedOffset::east_opt(7 * 3600).unwrap();
    let dt_wib = wib_offset.from_local_datetime(&naive).unwrap();
    
    Ok(dt_wib.to_rfc3339())
}

/// Basic normalization for dedupe: trim, unicode normalize (NFKC), lowercase.
pub fn normalize_name(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.trim()
        .nfkc()
        .collect::<String>()
        .to_lowercase()
}