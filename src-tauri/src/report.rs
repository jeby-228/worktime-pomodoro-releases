use chrono::{DateTime, Local, NaiveDate};
use std::collections::BTreeMap;

use crate::models::{
    Category, CategoryReport, CategoryReportDay, CategoryReportDayItem, CategoryReportRow,
    SlotType, TimeSlot,
};

#[derive(Default, Clone, Copy)]
struct Tally {
    seconds: i64,
    interrupt_seconds: i64,
}

impl Tally {
    fn add(&mut self, seconds: i64, is_interrupt: bool) {
        self.seconds += seconds;
        if is_interrupt {
            self.interrupt_seconds += seconds;
        }
    }
}

type Span = (DateTime<Local>, DateTime<Local>);

fn slot_span(slot: &TimeSlot, now: DateTime<Local>) -> Option<Span> {
    let end = slot.end_ts.unwrap_or(now);
    (end > slot.start_ts).then_some((slot.start_ts, end))
}

fn overlap_seconds(a: Span, b: Span) -> i64 {
    let start = a.0.max(b.0);
    let end = a.1.min(b.1);
    end.signed_duration_since(start).num_seconds().max(0)
}

/// Spans of the interrupts that started while another slot was open.
pub(crate) fn nested_interrupt_spans(slots: &[TimeSlot], now: DateTime<Local>) -> Vec<Span> {
    slots
        .iter()
        .filter(|slot| slot.slot_type == SlotType::Interrupt && slot.parent_id.is_some())
        .filter_map(|slot| slot_span(slot, now))
        .collect()
}

/// Seconds a slot really took. A task stays open in the database while an
/// interrupt runs, so its span covers the interrupt too; that overlap is taken
/// off here, otherwise the same time would count twice. Every overlapping slot
/// loses it, not just the parent: splitting a slot leaves the interrupt pointing
/// at the first half. Ongoing slots count up to `now`.
pub(crate) fn own_seconds(
    slot: &TimeSlot,
    nested_interrupts: &[Span],
    now: DateTime<Local>,
) -> i64 {
    let Some(span) = slot_span(slot, now) else {
        return 0;
    };
    let seconds = span.1.signed_duration_since(span.0).num_seconds();
    if slot.slot_type == SlotType::Interrupt {
        return seconds;
    }
    let overlap: i64 = nested_interrupts
        .iter()
        .map(|interrupt| overlap_seconds(span, *interrupt))
        .sum();
    (seconds - overlap).max(0)
}

/// Groups slots by category and by the day they started on, counting each slot
/// by its [`own_seconds`].
pub fn build_category_report(
    slots: &[TimeSlot],
    categories: &[Category],
    start: NaiveDate,
    end: NaiveDate,
    now: DateTime<Local>,
) -> CategoryReport {
    let nested_interrupts = nested_interrupt_spans(slots, now);

    let mut totals: BTreeMap<Option<i64>, Tally> = BTreeMap::new();
    let mut by_day: BTreeMap<NaiveDate, BTreeMap<Option<i64>, Tally>> = BTreeMap::new();

    for slot in slots {
        if !slot.slot_type.has_category() {
            continue;
        }
        let is_interrupt = slot.slot_type == SlotType::Interrupt;
        let seconds = own_seconds(slot, &nested_interrupts, now);
        if seconds <= 0 {
            continue;
        }
        totals
            .entry(slot.category_id)
            .or_default()
            .add(seconds, is_interrupt);
        by_day
            .entry(slot.start_ts.date_naive())
            .or_default()
            .entry(slot.category_id)
            .or_default()
            .add(seconds, is_interrupt);
    }

    let total_seconds: i64 = totals.values().map(|tally| tally.seconds).sum();

    // Display order for day stacks; uncategorized always goes last.
    let position = |id: Option<i64>| {
        id.map_or(usize::MAX, |id| {
            categories
                .iter()
                .position(|category| category.id == id)
                .unwrap_or(usize::MAX - 1)
        })
    };

    let mut rows: Vec<CategoryReportRow> = totals
        .iter()
        .map(|(id, tally)| CategoryReportRow {
            category_id: *id,
            name: id.and_then(|id| {
                categories
                    .iter()
                    .find(|category| category.id == id)
                    .map(|category| category.name.clone())
            }),
            total_seconds: tally.seconds,
            interrupt_seconds: tally.interrupt_seconds,
            percentage: tally.seconds as f64 / total_seconds as f64 * 100.0,
        })
        .collect();
    rows.sort_by_key(|row| (-row.total_seconds, position(row.category_id)));

    let mut days = Vec::new();
    let mut date = start;
    while date <= end {
        let mut items: Vec<CategoryReportDayItem> = by_day
            .get(&date)
            .map(|day| {
                day.iter()
                    .map(|(id, tally)| CategoryReportDayItem {
                        category_id: *id,
                        seconds: tally.seconds,
                        interrupt_seconds: tally.interrupt_seconds,
                    })
                    .collect()
            })
            .unwrap_or_default();
        items.sort_by_key(|item| position(item.category_id));
        days.push(CategoryReportDay {
            date: date.to_string(),
            total_seconds: items.iter().map(|item| item.seconds).sum(),
            items,
        });
        date = match date.succ_opt() {
            Some(next) => next,
            None => break,
        };
    }

    CategoryReport {
        start_date: start.to_string(),
        end_date: end.to_string(),
        total_seconds,
        rows,
        days,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn at(day: u32, hour: u32, minute: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 10, day, hour, minute, 0)
            .unwrap()
    }

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    fn category(id: i64, name: &str) -> Category {
        Category {
            id,
            name: name.to_string(),
            report_code: None,
            sort_order: id,
            hidden: false,
        }
    }

    fn categories() -> Vec<Category> {
        vec![category(1, "實作"), category(2, "問題處理")]
    }

    fn slot(
        slot_type: SlotType,
        category_id: Option<i64>,
        start: DateTime<Local>,
        minutes: i64,
    ) -> TimeSlot {
        let mut slot = TimeSlot::new("任務".to_string(), slot_type, start);
        slot.category_id = category_id;
        slot.end_ts = Some(start + Duration::minutes(minutes));
        slot
    }

    fn report(slots: &[TimeSlot]) -> CategoryReport {
        build_category_report(slots, &categories(), date(19), date(25), at(25, 23, 0))
    }

    fn hours(hours: i64) -> i64 {
        hours * 3600
    }

    #[test]
    fn test_totals_and_percentages_by_category() {
        let result = report(&[
            slot(SlotType::Work, Some(1), at(19, 9, 0), 180),
            slot(SlotType::Meeting, Some(2), at(19, 13, 0), 60),
        ]);

        assert_eq!(result.total_seconds, hours(4));
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].name.as_deref(), Some("實作"));
        assert_eq!(result.rows[0].total_seconds, hours(3));
        assert_eq!(result.rows[0].percentage, 75.0);
        assert_eq!(result.rows[1].name.as_deref(), Some("問題處理"));
        assert_eq!(result.rows[1].percentage, 25.0);
    }

    #[test]
    fn test_lunch_is_left_out() {
        let result = report(&[
            slot(SlotType::Work, Some(1), at(19, 9, 0), 60),
            slot(SlotType::Lunch, None, at(19, 12, 0), 60),
        ]);

        assert_eq!(result.total_seconds, hours(1));
        assert_eq!(result.rows.len(), 1);
    }

    #[test]
    fn test_old_slots_count_as_uncategorized() {
        let result = report(&[
            slot(SlotType::Work, None, at(19, 9, 0), 120),
            slot(SlotType::Work, Some(1), at(19, 13, 0), 120),
        ]);

        assert_eq!(result.rows[0].category_id, Some(1));
        assert_eq!(result.rows[1].category_id, None);
        assert_eq!(result.rows[1].name, None);
        assert_eq!(result.rows[1].total_seconds, hours(2));
    }

    #[test]
    fn test_interrupts_count_once_and_are_marked() {
        let mut work = slot(SlotType::Work, Some(1), at(19, 9, 0), 180);
        work.id = Some(1);
        let mut interrupt = slot(SlotType::Interrupt, Some(2), at(19, 10, 0), 60);
        interrupt.parent_id = Some(1);

        let result = report(&[work, interrupt]);

        // Three hours passed, so the report must say three hours.
        assert_eq!(result.total_seconds, hours(3));
        let build = &result.rows[0];
        assert_eq!(build.category_id, Some(1));
        assert_eq!(build.total_seconds, hours(2));
        assert_eq!(build.interrupt_seconds, 0);
        let fix = &result.rows[1];
        assert_eq!(fix.total_seconds, hours(1));
        assert_eq!(fix.interrupt_seconds, hours(1));
    }

    #[test]
    fn test_interrupt_in_the_tasks_own_category_is_still_marked() {
        let mut work = slot(SlotType::Work, Some(1), at(19, 9, 0), 120);
        work.id = Some(1);
        let mut interrupt = slot(SlotType::Interrupt, Some(1), at(19, 10, 0), 30);
        interrupt.parent_id = Some(1);

        let result = report(&[work, interrupt]);

        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].total_seconds, hours(2));
        assert_eq!(result.rows[0].interrupt_seconds, 1800);
    }

    #[test]
    fn test_standalone_interrupt_is_not_taken_off_other_slots() {
        let result = report(&[
            slot(SlotType::Work, Some(1), at(19, 9, 0), 60),
            slot(SlotType::Interrupt, Some(2), at(19, 9, 30), 60),
        ]);

        assert_eq!(result.total_seconds, hours(2));
    }

    #[test]
    fn test_ongoing_slot_counts_up_to_now() {
        let mut running = slot(SlotType::Work, Some(1), at(25, 22, 0), 0);
        running.end_ts = None;

        assert_eq!(report(&[running]).total_seconds, hours(1));
    }

    #[test]
    fn test_days_cover_the_whole_range_in_display_order() {
        let result = report(&[
            slot(SlotType::Work, None, at(20, 9, 0), 60),
            slot(SlotType::Work, Some(2), at(20, 11, 0), 60),
            slot(SlotType::Work, Some(1), at(20, 13, 0), 30),
        ]);

        assert_eq!(result.days.len(), 7);
        assert_eq!(result.days[0].date, "2026-10-19");
        assert!(result.days[0].items.is_empty());
        let day = &result.days[1];
        assert_eq!(day.total_seconds, 9000);
        let order: Vec<Option<i64>> = day.items.iter().map(|item| item.category_id).collect();
        assert_eq!(order, [Some(1), Some(2), None]);
    }

    #[test]
    fn test_empty_range_has_no_rows() {
        let result = report(&[]);

        assert_eq!(result.total_seconds, 0);
        assert!(result.rows.is_empty());
        assert_eq!(result.days.len(), 7);
    }
}
