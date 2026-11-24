use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Schedule {
    pub id: i64,
    pub job_id: i64,
    pub enabled: bool,

    pub cron_expression: String,
    pub schedule_type: Option<String>,
    pub time_of_day: Option<String>,
    pub day_of_week: Option<i32>,
    pub day_of_month: Option<i32>,

    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateScheduleRequest {
    pub enabled: Option<bool>,
    pub cron_expression: Option<String>,
    pub schedule_type: Option<String>,
    pub time_of_day: Option<String>,
    pub day_of_week: Option<i32>,
    pub day_of_month: Option<i32>,
}

impl CreateScheduleRequest {
    /// Convert simple schedule to cron expression
    pub fn to_cron_expression(&self) -> String {
        if let Some(cron) = &self.cron_expression {
            return cron.clone();
        }

        let time = self.time_of_day.as_deref().unwrap_or("00:00");
        let parts: Vec<&str> = time.split(':').collect();
        let hour = parts.first().unwrap_or(&"0");
        let minute = parts.get(1).unwrap_or(&"0");

        match self.schedule_type.as_deref() {
            Some("daily") => format!("{} {} * * *", minute, hour),
            Some("weekly") => {
                let dow = self.day_of_week.unwrap_or(0);
                format!("{} {} * * {}", minute, hour, dow)
            }
            Some("monthly") => {
                let dom = self.day_of_month.unwrap_or(1);
                format!("{} {} {} * *", minute, hour, dom)
            }
            _ => format!("{} {} * * *", minute, hour), // Default to daily
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateScheduleRequest {
    pub enabled: Option<bool>,
    pub cron_expression: Option<String>,
    pub schedule_type: Option<String>,
    pub time_of_day: Option<String>,
    pub day_of_week: Option<i32>,
    pub day_of_month: Option<i32>,
}

/// Parse a stored cron expression with the `cron` crate.
///
/// Stored expressions use standard 5-field Unix cron syntax
/// (`min hour dom month dow`, day of week 0-7 with 0 and 7 = Sunday), which
/// is what the UI builds and displays. The `cron` crate instead expects a
/// leading seconds field and numbers days of week 1-7 with 1 = Sunday, so
/// 5-field expressions are translated first. Expressions with 6 or 7 fields
/// are already in the crate's own format and are parsed unchanged.
pub fn parse_cron(expr: &str) -> Result<cron::Schedule, cron::error::Error> {
    use std::str::FromStr;
    cron::Schedule::from_str(&to_cron_crate_expression(expr))
}

/// Translate a standard 5-field cron expression to the `cron` crate format
/// (see [`parse_cron`]). Other expressions are returned unchanged.
pub fn to_cron_crate_expression(expr: &str) -> String {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() != 5 {
        return expr.to_string();
    }
    format!(
        "0 {} {} {} {} {}",
        fields[0],
        fields[1],
        fields[2],
        fields[3],
        convert_day_of_week_field(fields[4])
    )
}

/// Convert a standard cron day-of-week field (0-7, 0 and 7 = Sunday) to the
/// `cron` crate's numbering (1-7, 1 = Sunday).
///
/// Numeric items (`n`, `a-b`, `*/n`, `a/n`, `a-b/n`) are expanded and
/// re-emitted as an explicit list, which also handles ranges ending on 7.
/// Wildcards, `?`, day names and anything that is not a valid numeric item
/// are passed through for the crate to accept or reject.
fn convert_day_of_week_field(field: &str) -> String {
    if field == "*" || field == "?" {
        return field.to_string();
    }

    let mut out: Vec<String> = Vec::new();
    for item in field.split(',') {
        match expand_standard_dow_item(item) {
            Some(days) => {
                for day in days {
                    // 0 and 7 are both Sunday in standard cron
                    let crate_day = if day == 7 { 1 } else { day + 1 };
                    let s = crate_day.to_string();
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
            None => out.push(item.to_string()),
        }
    }
    out.join(",")
}

/// Expand one numeric day-of-week list item in standard cron numbering.
/// Returns `None` for items that are not purely numeric or are out of range.
fn expand_standard_dow_item(item: &str) -> Option<Vec<u32>> {
    let (range, step) = match item.split_once('/') {
        Some((range, step)) => (range, Some(step.parse::<u32>().ok().filter(|s| *s > 0)?)),
        None => (item, None),
    };

    let (start, end) = if range == "*" {
        (0, 6)
    } else if let Some((a, b)) = range.split_once('-') {
        (a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)
    } else {
        let a = range.parse::<u32>().ok()?;
        // "a/n" means "from a to the end of the range, every n"
        (a, if step.is_some() { 6 } else { a })
    };

    if start > 7 || end > 7 || start > end {
        return None;
    }

    Some((start..=end).step_by(step.unwrap_or(1) as usize).collect())
}

/// Longest time between two consecutive runs of a schedule, measured over its
/// next runs in `tz` (so weekday lists, short months and DST are included).
/// None for an invalid expression or one with fewer than two upcoming runs.
pub fn max_schedule_gap(cron_expr: &str, tz: chrono_tz::Tz) -> Option<chrono::Duration> {
    let schedule = parse_cron(cron_expr).ok()?;
    let times: Vec<_> = schedule.upcoming(tz).take(64).collect();
    times.windows(2).map(|w| w[1] - w[0]).max()
}

/// The overdue limit for a job's enabled schedules: twice the gap of its most
/// frequent schedule plus an hour (one missed run is tolerated, two are not).
/// Shared by the dashboard (via the job list) and the overdue notifications.
pub fn overdue_after<'a>(
    cron_exprs: impl IntoIterator<Item = &'a str>,
    tz: chrono_tz::Tz,
) -> Option<chrono::Duration> {
    cron_exprs
        .into_iter()
        .filter_map(|c| max_schedule_gap(c, tz))
        .min()
        .map(|gap| gap * 2 + chrono::Duration::hours(1))
}

#[cfg(test)]
mod gap_tests {
    use super::*;
    use chrono::Duration;
    use chrono_tz::Tz;

    #[test]
    fn measures_the_longest_gap() {
        assert_eq!(max_schedule_gap("*/15 * * * *", Tz::UTC), Some(Duration::minutes(15)));
        assert_eq!(max_schedule_gap("30 2 * * *", Tz::UTC), Some(Duration::days(1)));
        // Mon + Fri: Friday to Monday
        assert_eq!(max_schedule_gap("0 2 * * 1,5", Tz::UTC), Some(Duration::days(4)));
        // Weekdays only: Friday to Monday
        assert_eq!(max_schedule_gap("0 2 * * 1-5", Tz::UTC), Some(Duration::days(3)));
        // Day 31 skips short months
        assert!(max_schedule_gap("0 3 31 * *", Tz::UTC).unwrap() >= Duration::days(61));
        assert_eq!(max_schedule_gap("not cron", Tz::UTC), None);
    }

    #[test]
    fn overdue_limit_uses_the_most_frequent_schedule() {
        let limit = overdue_after(["0 3 1 * *", "0 2 * * *"], Tz::UTC);
        assert_eq!(limit, Some(Duration::days(2) + Duration::hours(1)));
        assert_eq!(overdue_after(["bogus"], Tz::UTC), None);
        assert_eq!(overdue_after([], Tz::UTC), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Weekday};

    fn next_weekdays(expr: &str, count: usize) -> Vec<Weekday> {
        // Saturday 2026-01-03 00:00 UTC as a fixed reference point
        let start = Utc.with_ymd_and_hms(2026, 1, 3, 0, 0, 0).unwrap();
        parse_cron(expr)
            .unwrap()
            .after(&start)
            .take(count)
            .map(|dt| dt.weekday())
            .collect()
    }

    #[test]
    fn test_five_field_gets_seconds_prefix() {
        assert_eq!(to_cron_crate_expression("30 2 * * *"), "0 30 2 * * *");
    }

    #[test]
    fn test_six_and_seven_field_unchanged() {
        assert_eq!(to_cron_crate_expression("0 30 2 * * 2"), "0 30 2 * * 2");
        assert_eq!(to_cron_crate_expression("0 30 2 * * 2 *"), "0 30 2 * * 2 *");
    }

    #[test]
    fn test_day_of_week_conversion() {
        let cases = [
            ("*", "*"),
            ("?", "?"),
            ("0", "1"),
            ("1", "2"),
            ("6", "7"),
            ("7", "1"),
            ("1-5", "2,3,4,5,6"),
            ("0-6", "1,2,3,4,5,6,7"),
            ("5-7", "6,7,1"),
            ("1,3,5", "2,4,6"),
            ("0,7", "1"),
            ("*/2", "1,3,5,7"),
            ("1-5/2", "2,4,6"),
            ("1/3", "2,5"),
            ("MON-FRI", "MON-FRI"),
            ("1,SAT", "2,SAT"),
        ];
        for (input, expected) in cases {
            assert_eq!(convert_day_of_week_field(input), expected, "input {input}");
        }
    }

    #[test]
    fn test_invalid_items_passed_through() {
        assert_eq!(convert_day_of_week_field("8"), "8");
        assert_eq!(convert_day_of_week_field("5-1"), "5-1");
        assert_eq!(convert_day_of_week_field("*/0"), "*/0");
        assert!(parse_cron("0 2 * * 8").is_err());
    }

    #[test]
    fn test_weekly_schedules_run_on_intended_day() {
        assert_eq!(next_weekdays("0 2 * * 0", 1), vec![Weekday::Sun]);
        assert_eq!(next_weekdays("0 2 * * 1", 1), vec![Weekday::Mon]);
        assert_eq!(next_weekdays("0 2 * * 6", 1), vec![Weekday::Sat]);
        assert_eq!(next_weekdays("0 2 * * 7", 1), vec![Weekday::Sun]);
        assert_eq!(
            next_weekdays("0 2 * * 1-5", 5),
            vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri]
        );
        assert_eq!(next_weekdays("0 2 * * MON", 1), vec![Weekday::Mon]);
    }

    #[test]
    fn test_create_request_weekly_sunday_is_valid() {
        let req = CreateScheduleRequest {
            enabled: None,
            cron_expression: None,
            schedule_type: Some("weekly".into()),
            time_of_day: Some("03:00".into()),
            day_of_week: Some(0),
            day_of_month: None,
        };
        let expr = req.to_cron_expression();
        assert_eq!(next_weekdays(&expr, 1), vec![Weekday::Sun]);
    }
}
