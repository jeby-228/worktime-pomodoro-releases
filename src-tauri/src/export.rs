use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Local, NaiveDate, Timelike};
use rust_xlsxwriter::{Color, Format, FormatAlign, Workbook};
use std::path::Path;

use crate::store::{Segment, SegmentStatus};

pub const HEADER_DATE: &str = "日期";
pub const HEADER_START: &str = "開始時間";
pub const HEADER_END: &str = "結束時間";
pub const HEADER_MINUTES: &str = "持續分鐘數";
pub const HEADER_TASK: &str = "任務名稱";
pub const PENDING_LABEL: &str = "待確認";
pub const TOTAL_LABEL: &str = "合計";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRow {
    pub date: NaiveDate,
    pub start_time: String,
    pub end_time: String,
    pub minutes: i64,
    pub task_name: String,
}

/// Round a duration to the nearest integer minute (halfway cases away from zero).
pub fn round_minutes(start: DateTime<Local>, end: DateTime<Local>) -> i64 {
    let millis = end.signed_duration_since(start).num_milliseconds();
    if millis <= 0 {
        return 0;
    }
    let minutes = millis as f64 / 60_000.0;
    minutes.round() as i64
}

/// Split a half-open [start, end) span across calendar days.
/// The last instant of a day is represented as 00:00 on the next local midnight
/// for the end time of that day's row.
pub fn split_segment_by_date(
    start: DateTime<Local>,
    end: DateTime<Local>,
) -> Vec<(NaiveDate, DateTime<Local>, DateTime<Local>)> {
    if end <= start {
        return Vec::new();
    }

    let mut parts = Vec::new();
    let mut cursor = start;

    while cursor.date_naive() < end.date_naive() {
        let next_midnight = next_local_midnight(cursor);
        if next_midnight > cursor {
            parts.push((cursor.date_naive(), cursor, next_midnight));
        }
        cursor = next_midnight;
        if parts.len() > 366 {
            break;
        }
    }

    if end > cursor {
        parts.push((cursor.date_naive(), cursor, end));
    }

    parts
}

fn next_local_midnight(ts: DateTime<Local>) -> DateTime<Local> {
    let tomorrow = ts.date_naive() + Duration::days(1);
    tomorrow
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight")
        .and_local_timezone(Local)
        .earliest()
        .unwrap_or_else(|| ts + Duration::hours(24 - ts.hour() as i64))
}

fn format_hhmm(ts: DateTime<Local>) -> String {
    format!("{:02}:{:02}", ts.hour(), ts.minute())
}

fn task_name_for(segment: &Segment, include_pending_mark: bool) -> String {
    let base = segment
        .task_key
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(PENDING_LABEL);

    if include_pending_mark && segment.status == SegmentStatus::Unknown {
        if base == PENDING_LABEL {
            PENDING_LABEL.to_string()
        } else {
            format!("{base}（{PENDING_LABEL}）")
        }
    } else {
        base.to_string()
    }
}

/// Build export rows from stored segments.
///
/// Pending/unresolved (`Unknown`) segments are skipped unless `include_pending`.
/// Open segments (no `end_ts`) use `now` as the end. Cross-midnight work is
/// split into one row per calendar date. Rows outside `[from_date, to_date]`
/// are dropped. Result is sorted by start datetime.
pub fn build_export_rows(
    segments: &[Segment],
    from_date: NaiveDate,
    to_date: NaiveDate,
    include_pending: bool,
    now: DateTime<Local>,
) -> Vec<ExportRow> {
    let mut rows = Vec::new();

    for segment in segments {
        if segment.status == SegmentStatus::Unknown && !include_pending {
            continue;
        }

        let end = match segment.end_ts {
            Some(ts) => ts,
            None => now,
        };
        if end <= segment.start_ts {
            continue;
        }

        let mark_pending = segment.status == SegmentStatus::Unknown;
        let task_name = task_name_for(segment, mark_pending);

        for (date, start, part_end) in split_segment_by_date(segment.start_ts, end) {
            if date < from_date || date > to_date {
                continue;
            }
            rows.push(ExportRow {
                date,
                start_time: format_hhmm(start),
                end_time: format_hhmm(part_end),
                minutes: round_minutes(start, part_end),
                task_name: task_name.clone(),
            });
        }
    }

    rows.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.start_time.cmp(&b.start_time))
            .then_with(|| a.end_time.cmp(&b.end_time))
    });

    rows
}

pub fn default_xlsx_filename(from_date: NaiveDate, to_date: NaiveDate) -> String {
    format!(
        "工時_{}-{}.xlsx",
        from_date.format("%Y%m%d"),
        to_date.format("%Y%m%d")
    )
}

pub fn write_xlsx<P: AsRef<Path>>(path: P, rows: &[ExportRow]) -> Result<i64> {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("工時")?;

    let header_fmt = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4A5568))
        .set_font_color(Color::RGB(0xFFFFFF))
        .set_align(FormatAlign::Center);

    let total_fmt = Format::new().set_bold();
    let date_fmt = Format::new().set_align(FormatAlign::Center);
    let time_fmt = Format::new().set_align(FormatAlign::Center);
    let minutes_fmt = Format::new().set_align(FormatAlign::Right);

    worksheet.write_with_format(0, 0, HEADER_DATE, &header_fmt)?;
    worksheet.write_with_format(0, 1, HEADER_START, &header_fmt)?;
    worksheet.write_with_format(0, 2, HEADER_END, &header_fmt)?;
    worksheet.write_with_format(0, 3, HEADER_MINUTES, &header_fmt)?;
    worksheet.write_with_format(0, 4, HEADER_TASK, &header_fmt)?;

    worksheet.set_column_width(0, 14)?;
    worksheet.set_column_width(1, 12)?;
    worksheet.set_column_width(2, 12)?;
    worksheet.set_column_width(3, 16)?;
    worksheet.set_column_width(4, 42)?;
    worksheet.set_freeze_panes(1, 0)?;

    let mut total_minutes: i64 = 0;
    for (idx, row) in rows.iter().enumerate() {
        let r = (idx + 1) as u32;
        worksheet.write_with_format(r, 0, row.date.format("%Y-%m-%d").to_string(), &date_fmt)?;
        worksheet.write_with_format(r, 1, &row.start_time, &time_fmt)?;
        worksheet.write_with_format(r, 2, &row.end_time, &time_fmt)?;
        worksheet.write_number_with_format(r, 3, row.minutes as f64, &minutes_fmt)?;
        worksheet.write_string(r, 4, &row.task_name)?;
        total_minutes += row.minutes;
    }

    let total_row = (rows.len() + 1) as u32;
    worksheet.write_with_format(total_row, 0, TOTAL_LABEL, &total_fmt)?;
    worksheet.write_number_with_format(total_row, 3, total_minutes as f64, &total_fmt)?;

    workbook.save(path.as_ref())?;
    Ok(total_minutes)
}

pub fn parse_iso_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| anyhow::anyhow!("無效的日期「{s}」: {e}"))
}

pub fn validate_range(from_date: NaiveDate, to_date: NaiveDate) -> Result<()> {
    if from_date > to_date {
        bail!(
            "起始日期 {} 晚於結束日期 {}",
            from_date.format("%Y-%m-%d"),
            to_date.format("%Y-%m-%d")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Segment;
    use chrono::TimeZone;

    fn local_ymd_hms(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        min: u32,
        sec: u32,
    ) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(year, month, day, hour, min, sec)
            .single()
            .unwrap_or_else(|| {
                DateTime::from_naive_utc_and_offset(
                    NaiveDate::from_ymd_opt(year, month, day)
                        .unwrap()
                        .and_hms_opt(hour, min, sec)
                        .unwrap(),
                    *Local::now().offset(),
                )
            })
    }

    fn segment(
        task: Option<&str>,
        start: DateTime<Local>,
        end: Option<DateTime<Local>>,
        status: SegmentStatus,
    ) -> Segment {
        Segment {
            id: None,
            task_key: task.map(|s| s.to_string()),
            start_ts: start,
            end_ts: end,
            minutes: end.map(|e| round_minutes(start, e)),
            status,
            synced: false,
        }
    }

    #[test]
    fn test_minutes_rounding() {
        let start = local_ymd_hms(2026, 10, 21, 9, 0, 0);
        // 29.4s → 0 minutes
        assert_eq!(
            round_minutes(start, start + Duration::milliseconds(29_400)),
            0
        );
        // 30s → 1 minute (half away from zero / nearest)
        assert_eq!(round_minutes(start, start + Duration::seconds(30)), 1);
        // 89s → 1 minute
        assert_eq!(round_minutes(start, start + Duration::seconds(89)), 1);
        // 90s → 2 minutes
        assert_eq!(round_minutes(start, start + Duration::seconds(90)), 2);
        // exact hour
        assert_eq!(round_minutes(start, start + Duration::hours(1)), 60);
        // inverted range
        assert_eq!(round_minutes(start + Duration::hours(1), start), 0);
    }

    #[test]
    fn test_cross_midnight_segment_split_by_date() {
        let start = local_ymd_hms(2026, 10, 21, 23, 10, 0);
        let end = local_ymd_hms(2026, 10, 22, 1, 5, 0);
        let parts = split_segment_by_date(start, end);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0, NaiveDate::from_ymd_opt(2026, 10, 21).unwrap());
        assert_eq!(parts[1].0, NaiveDate::from_ymd_opt(2026, 10, 22).unwrap());
        assert_eq!(format_hhmm(parts[0].1), "23:10");
        assert_eq!(format_hhmm(parts[0].2), "00:00");
        assert_eq!(format_hhmm(parts[1].1), "00:00");
        assert_eq!(format_hhmm(parts[1].2), "01:05");
        assert_eq!(round_minutes(parts[0].1, parts[0].2), 50);
        assert_eq!(round_minutes(parts[1].1, parts[1].2), 65);
    }

    #[test]
    fn test_build_rows_excludes_pending_by_default() {
        let from = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let confirmed = segment(
            Some("撰寫規格"),
            local_ymd_hms(2026, 10, 21, 9, 0, 0),
            Some(local_ymd_hms(2026, 10, 21, 10, 30, 0)),
            SegmentStatus::Confirmed,
        );
        let pending = segment(
            None,
            local_ymd_hms(2026, 10, 21, 11, 0, 0),
            Some(local_ymd_hms(2026, 10, 21, 11, 20, 0)),
            SegmentStatus::Unknown,
        );
        let now = local_ymd_hms(2026, 10, 21, 18, 0, 0);

        let rows = build_export_rows(
            &[confirmed.clone(), pending.clone()],
            from,
            from,
            false,
            now,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].task_name, "撰寫規格");
        assert_eq!(rows[0].minutes, 90);

        let with_pending = build_export_rows(&[confirmed, pending], from, from, true, now);
        assert_eq!(with_pending.len(), 2);
        assert_eq!(with_pending[1].task_name, PENDING_LABEL);
        assert_eq!(with_pending[1].minutes, 20);
    }

    #[test]
    fn test_build_rows_splits_overnight_and_sorts() {
        let from = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        let later = segment(
            Some("代碼審查"),
            local_ymd_hms(2026, 10, 22, 9, 0, 0),
            Some(local_ymd_hms(2026, 10, 22, 9, 45, 0)),
            SegmentStatus::Confirmed,
        );
        let overnight = segment(
            Some("值班｜深夜部署"),
            local_ymd_hms(2026, 10, 21, 23, 0, 0),
            Some(local_ymd_hms(2026, 10, 22, 1, 0, 0)),
            SegmentStatus::Confirmed,
        );
        let now = local_ymd_hms(2026, 10, 22, 18, 0, 0);

        let rows = build_export_rows(&[later, overnight], from, to, false, now);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].date, from);
        assert_eq!(rows[0].start_time, "23:00");
        assert_eq!(rows[0].end_time, "00:00");
        assert_eq!(rows[0].minutes, 60);
        assert_eq!(rows[0].task_name, "值班｜深夜部署");
        assert_eq!(rows[1].date, to);
        assert_eq!(rows[1].start_time, "00:00");
        assert_eq!(rows[1].end_time, "01:00");
        assert_eq!(rows[1].minutes, 60);
        assert_eq!(rows[2].task_name, "代碼審查");
        assert_eq!(rows[2].minutes, 45);
    }

    #[test]
    fn test_default_filename() {
        let from = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        assert_eq!(
            default_xlsx_filename(from, to),
            "工時_20261001-20261021.xlsx"
        );
    }

    #[test]
    fn test_write_xlsx_headers_total_and_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.xlsx");
        let rows = vec![ExportRow {
            date: NaiveDate::from_ymd_opt(2026, 10, 21).unwrap(),
            start_time: "09:00".to_string(),
            end_time: "10:25".to_string(),
            minutes: 85,
            task_name: "計劃A｜開發功能".to_string(),
        }];
        let total = write_xlsx(&path, &rows).unwrap();
        assert_eq!(total, 85);
        assert!(path.exists());
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], b"PK", "xlsx is a zip container");
    }

    #[test]
    fn test_validate_range() {
        let a = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let b = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        assert!(validate_range(a, b).is_ok());
        assert!(validate_range(b, a).is_err());
    }
}
