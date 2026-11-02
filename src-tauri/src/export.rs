use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::models::{SlotType, TimeSlot};
use crate::report::{nested_interrupt_spans, own_seconds};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportRowV2 {
    pub date: String,
    pub task_name: String,
    /// Status and category together, such as 工作：實作. Lunch is just its status.
    pub status_category: String,
    pub slot_type: String,
    pub category: String,
    pub report_code: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_minutes: i64,
    pub note: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ExportTypeLabels {
    #[serde(default)]
    pub work: String,
    #[serde(default)]
    pub interrupt: String,
    #[serde(default)]
    pub meeting: String,
    #[serde(default)]
    pub lunch: String,
    #[serde(default)]
    pub uncategorized: String,
}

impl ExportTypeLabels {
    fn for_slot(&self, slot_type: &SlotType) -> String {
        let label = match slot_type {
            SlotType::Work => self.work.as_str(),
            SlotType::Interrupt => self.interrupt.as_str(),
            SlotType::Meeting => self.meeting.as_str(),
            SlotType::Lunch => self.lunch.as_str(),
        };
        let label = label.trim();
        if label.is_empty() {
            slot_type.as_str().to_string()
        } else {
            label.to_string()
        }
    }

    fn uncategorized(&self) -> String {
        let label = self.uncategorized.trim();
        if label.is_empty() {
            "未分類".to_string()
        } else {
            label.to_string()
        }
    }
}

/// Finished slots become rows. A slot still running has no end time to write.
///
/// Minutes are counted the way the weekly report counts them, so a task does
/// not repeat the time of its interrupts. Start and end stay as recorded. That
/// holds with `include_interrupts` off too: leaving the interrupt rows out does
/// not hand their time back to the task.
pub fn build_export_rows(
    slots: Vec<TimeSlot>,
    include_interrupts: bool,
    labels: &ExportTypeLabels,
) -> Vec<ExportRowV2> {
    build_export_rows_at(slots, include_interrupts, labels, Local::now())
}

fn build_export_rows_at(
    slots: Vec<TimeSlot>,
    include_interrupts: bool,
    labels: &ExportTypeLabels,
    now: DateTime<Local>,
) -> Vec<ExportRowV2> {
    let nested_interrupts = nested_interrupt_spans(&slots, now);

    slots
        .into_iter()
        .filter(|slot| include_interrupts || slot.slot_type != SlotType::Interrupt)
        .filter_map(|slot| {
            let end_ts = slot.end_ts?;
            let duration = own_seconds(&slot, &nested_interrupts, now) / 60;
            let status = labels.for_slot(&slot.slot_type);
            let category = if slot.slot_type.has_category() {
                slot.category_name.unwrap_or_else(|| labels.uncategorized())
            } else {
                String::new()
            };
            let status_category = if category.is_empty() {
                status.clone()
            } else {
                format!("{status}：{category}")
            };

            Some(ExportRowV2 {
                date: slot.start_ts.format("%Y-%m-%d").to_string(),
                task_name: slot.task_name,
                status_category,
                slot_type: status,
                category,
                report_code: slot.category_code.unwrap_or_default(),
                start_time: slot.start_ts.format("%H:%M:%S").to_string(),
                end_time: end_ts.format("%H:%M:%S").to_string(),
                duration_minutes: duration,
                note: slot.note.unwrap_or_default(),
            })
        })
        .collect()
}

pub enum ExportCell<'a> {
    Text(&'a str),
    Number(i64),
}

impl ExportCell<'_> {
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(text) => text.to_string(),
            Self::Number(number) => number.to_string(),
        }
    }
}

pub struct ExportColumn {
    pub header: &'static str,
    pub width: u16,
    pub value: for<'a> fn(&'a ExportRowV2) -> ExportCell<'a>,
}

/// The columns shared by the CSV and Excel files. 報工代碼 only appears when a
/// row in this export has one.
pub fn export_columns(rows: &[ExportRowV2]) -> Vec<ExportColumn> {
    let mut columns = vec![
        ExportColumn {
            header: "日期",
            width: 12,
            value: |row| ExportCell::Text(&row.date),
        },
        ExportColumn {
            header: "任務",
            width: 30,
            value: |row| ExportCell::Text(&row.task_name),
        },
        ExportColumn {
            header: "狀態：類別",
            width: 18,
            value: |row| ExportCell::Text(&row.status_category),
        },
        ExportColumn {
            header: "狀態",
            width: 10,
            value: |row| ExportCell::Text(&row.slot_type),
        },
        ExportColumn {
            header: "類別",
            width: 14,
            value: |row| ExportCell::Text(&row.category),
        },
    ];
    if rows.iter().any(|row| !row.report_code.is_empty()) {
        columns.push(ExportColumn {
            header: "報工代碼",
            width: 12,
            value: |row| ExportCell::Text(&row.report_code),
        });
    }
    columns.extend([
        ExportColumn {
            header: "開始時間",
            width: 10,
            value: |row| ExportCell::Text(&row.start_time),
        },
        ExportColumn {
            header: "結束時間",
            width: 10,
            value: |row| ExportCell::Text(&row.end_time),
        },
        ExportColumn {
            header: "分鐘",
            width: 8,
            value: |row| ExportCell::Number(row.duration_minutes),
        },
        ExportColumn {
            header: "備註",
            width: 40,
            value: |row| ExportCell::Text(&row.note),
        },
    ]);
    columns
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Category;
    use crate::report::build_category_report;
    use chrono::{Duration, TimeZone};

    fn labels() -> ExportTypeLabels {
        ExportTypeLabels {
            work: "工作".to_string(),
            interrupt: "插隊".to_string(),
            meeting: "會議".to_string(),
            lunch: "午休".to_string(),
            uncategorized: "未分類".to_string(),
        }
    }

    fn finished(slot_type: SlotType, category: Option<(&str, Option<&str>)>) -> TimeSlot {
        let start = Local::now();
        let mut slot = TimeSlot::new("任務".to_string(), slot_type, start);
        slot.end_ts = Some(start + Duration::minutes(30));
        if let Some((name, code)) = category {
            slot.category_id = Some(1);
            slot.category_name = Some(name.to_string());
            slot.category_code = code.map(str::to_string);
        }
        slot
    }

    fn at(hour: u32, minute: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 10, 19, hour, minute, 0)
            .unwrap()
    }

    /// A finished slot with an id, category 1 and the given span.
    fn spanned(
        id: i64,
        slot_type: SlotType,
        start: DateTime<Local>,
        end: DateTime<Local>,
    ) -> TimeSlot {
        let mut slot = TimeSlot::new("任務".to_string(), slot_type, start);
        slot.id = Some(id);
        slot.end_ts = Some(end);
        slot.category_id = Some(1);
        slot
    }

    fn work(id: i64, start: DateTime<Local>, end: DateTime<Local>) -> TimeSlot {
        spanned(id, SlotType::Work, start, end)
    }

    fn interrupt(
        id: i64,
        parent_id: Option<i64>,
        start: DateTime<Local>,
        end: DateTime<Local>,
    ) -> TimeSlot {
        let mut slot = spanned(id, SlotType::Interrupt, start, end);
        slot.parent_id = parent_id;
        slot
    }

    fn minutes(slots: Vec<TimeSlot>) -> Vec<i64> {
        build_export_rows_at(slots, true, &labels(), at(23, 0))
            .iter()
            .map(|row| row.duration_minutes)
            .collect()
    }

    fn report_seconds(slots: &[TimeSlot]) -> i64 {
        let day = at(0, 0).date_naive();
        let categories: [Category; 0] = [];
        build_category_report(slots, &categories, day, day, at(23, 0)).total_seconds
    }

    fn headers(rows: &[ExportRowV2]) -> Vec<&'static str> {
        export_columns(rows)
            .iter()
            .map(|column| column.header)
            .collect()
    }

    #[test]
    fn test_status_and_category_are_written_together_and_apart() {
        let rows = build_export_rows(
            vec![finished(SlotType::Work, Some(("實作", None)))],
            true,
            &labels(),
        );

        assert_eq!(rows[0].status_category, "工作：實作");
        assert_eq!(rows[0].slot_type, "工作");
        assert_eq!(rows[0].category, "實作");
    }

    #[test]
    fn test_uncategorized_uses_the_label_from_the_app() {
        let rows = build_export_rows(vec![finished(SlotType::Meeting, None)], true, &labels());
        assert_eq!(rows[0].status_category, "會議：未分類");

        let plain = ExportTypeLabels::default();
        let rows = build_export_rows(vec![finished(SlotType::Work, None)], true, &plain);
        assert_eq!(rows[0].category, "未分類");
    }

    #[test]
    fn test_lunch_has_no_category() {
        let rows = build_export_rows(vec![finished(SlotType::Lunch, None)], true, &labels());

        assert_eq!(rows[0].status_category, "午休");
        assert_eq!(rows[0].category, "");
    }

    #[test]
    fn test_report_code_column_only_when_a_row_has_a_code() {
        let without = build_export_rows(
            vec![finished(SlotType::Work, Some(("實作", None)))],
            true,
            &labels(),
        );
        assert!(!headers(&without).contains(&"報工代碼"));

        let with = build_export_rows(
            vec![
                finished(SlotType::Work, Some(("實作", Some("A01")))),
                finished(SlotType::Work, Some(("文件維護", None))),
            ],
            true,
            &labels(),
        );
        assert_eq!(
            headers(&with),
            [
                "日期",
                "任務",
                "狀態：類別",
                "狀態",
                "類別",
                "報工代碼",
                "開始時間",
                "結束時間",
                "分鐘",
                "備註"
            ]
        );
        assert_eq!(with[0].report_code, "A01");
        assert_eq!(with[1].report_code, "");
    }

    #[test]
    fn test_interrupts_can_be_left_out() {
        let slots = vec![
            finished(SlotType::Work, None),
            finished(SlotType::Interrupt, None),
        ];
        assert_eq!(build_export_rows(slots, false, &labels()).len(), 1);
    }

    #[test]
    fn test_running_slots_are_skipped() {
        let mut running = finished(SlotType::Work, None);
        running.end_ts = None;
        assert!(build_export_rows(vec![running], true, &labels()).is_empty());
    }

    #[test]
    fn test_minutes_stay_numeric_for_excel() {
        let rows = build_export_rows(vec![finished(SlotType::Work, None)], true, &labels());
        let columns = export_columns(&rows);
        let minutes = columns
            .iter()
            .find(|column| column.header == "分鐘")
            .unwrap();
        assert!(matches!((minutes.value)(&rows[0]), ExportCell::Number(30)));
    }

    #[test]
    fn test_slots_without_interrupts_keep_their_full_length() {
        assert_eq!(
            minutes(vec![
                work(1, at(9, 0), at(12, 0)),
                work(2, at(13, 0), at(14, 0)),
            ]),
            [180, 60]
        );
    }

    #[test]
    fn test_interrupt_is_taken_off_its_task_and_stays_its_own_row() {
        let slots = vec![
            work(1, at(9, 0), at(12, 0)),
            interrupt(2, Some(1), at(10, 0), at(11, 0)),
        ];

        let rows = build_export_rows_at(slots, true, &labels(), at(23, 0));

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].duration_minutes, 120);
        assert_eq!(rows[0].start_time, "09:00:00");
        assert_eq!(rows[0].end_time, "12:00:00");
        assert_eq!(rows[1].slot_type, "插隊");
        assert_eq!(rows[1].duration_minutes, 60);
    }

    #[test]
    fn test_several_interrupts_all_come_off() {
        assert_eq!(
            minutes(vec![
                work(1, at(9, 0), at(12, 0)),
                interrupt(2, Some(1), at(9, 30), at(9, 45)),
                interrupt(3, Some(1), at(10, 0), at(11, 0)),
            ]),
            [105, 15, 60]
        );
    }

    #[test]
    fn test_only_the_part_inside_the_slot_comes_off() {
        // Past the end of the task.
        assert_eq!(
            minutes(vec![
                work(1, at(9, 0), at(10, 0)),
                interrupt(2, Some(1), at(9, 30), at(10, 30)),
            ]),
            [30, 60]
        );
        // Before the start of the task.
        assert_eq!(
            minutes(vec![
                interrupt(2, Some(1), at(8, 30), at(9, 30)),
                work(1, at(9, 0), at(10, 0)),
            ]),
            [60, 30]
        );
    }

    #[test]
    fn test_interrupt_covering_the_whole_slot_leaves_zero_not_negative() {
        assert_eq!(
            minutes(vec![
                work(1, at(9, 0), at(10, 0)),
                interrupt(2, Some(1), at(8, 0), at(11, 0)),
            ]),
            [0, 180]
        );
    }

    #[test]
    fn test_overlap_is_counted_in_seconds_before_rounding_down() {
        let start = at(9, 0);
        assert_eq!(
            minutes(vec![
                work(1, start, start + Duration::minutes(10)),
                interrupt(
                    2,
                    Some(1),
                    start + Duration::seconds(150),
                    start + Duration::seconds(180),
                ),
            ]),
            [9, 0]
        );
    }

    #[test]
    fn test_standalone_interrupt_is_not_taken_off() {
        assert_eq!(
            minutes(vec![
                work(1, at(9, 0), at(10, 0)),
                interrupt(2, None, at(9, 30), at(10, 30)),
            ]),
            [60, 60]
        );
    }

    #[test]
    fn test_second_half_of_a_split_task_loses_the_interrupt_too() {
        // split_slot leaves the interrupt's parent_id on the first half.
        assert_eq!(
            minutes(vec![
                work(1, at(9, 0), at(10, 30)),
                work(3, at(10, 30), at(12, 0)),
                interrupt(2, Some(1), at(11, 0), at(11, 30)),
            ]),
            [90, 60, 30]
        );
    }

    #[test]
    fn test_hiding_interrupts_does_not_give_their_time_back() {
        let slots = vec![
            work(1, at(9, 0), at(12, 0)),
            interrupt(2, Some(1), at(10, 0), at(11, 0)),
        ];

        let rows = build_export_rows_at(slots, false, &labels(), at(23, 0));

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].duration_minutes, 120);
    }

    #[test]
    fn test_running_interrupt_comes_off_up_to_now() {
        let mut running = interrupt(2, Some(1), at(9, 30), at(9, 30));
        running.end_ts = None;
        let slots = vec![work(1, at(9, 0), at(10, 0)), running];

        let rows = build_export_rows_at(slots, true, &labels(), at(9, 45));

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].duration_minutes, 45);
    }

    #[test]
    fn test_interrupted_lunch_does_not_repeat_the_interrupt() {
        assert_eq!(
            minutes(vec![
                spanned(1, SlotType::Lunch, at(12, 0), at(13, 0)),
                interrupt(2, Some(1), at(12, 10), at(12, 20)),
            ]),
            [50, 10]
        );
    }

    #[test]
    fn test_total_matches_the_weekly_report() {
        let slots = vec![
            work(1, at(9, 0), at(12, 0)),
            interrupt(2, Some(1), at(10, 0), at(11, 0)),
            spanned(3, SlotType::Lunch, at(12, 0), at(13, 0)),
            spanned(4, SlotType::Meeting, at(13, 0), at(14, 0)),
            work(5, at(14, 0), at(15, 0)),
            interrupt(6, Some(5), at(14, 30), at(15, 30)),
            interrupt(7, None, at(16, 0), at(16, 20)),
        ];
        let expected = report_seconds(&slots);

        let rows = build_export_rows_at(slots, true, &labels(), at(23, 0));

        // The report leaves lunch out, so its row is left out here as well.
        let total: i64 = rows
            .iter()
            .filter(|row| row.slot_type != "午休")
            .map(|row| row.duration_minutes)
            .sum();
        assert_eq!(total * 60, expected);
        assert_eq!(total, 120 + 60 + 60 + 30 + 60 + 20);
    }
}
