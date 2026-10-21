use crate::store::{Event, EventKind, Segment, SegmentStatus, Store};
use anyhow::Result;
use chrono::{DateTime, Local};
use std::collections::HashMap;

pub struct SegmentDeriver {
    store: Store,
}

impl SegmentDeriver {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    /// Derive all segments from events
    pub fn derive_all(&self) -> Result<Vec<Segment>> {
        let events = self.store.get_all_events()?;
        self.derive_from_events(&events)
    }

    /// Derive segments from a list of events
    pub fn derive_from_events(&self, events: &[Event]) -> Result<Vec<Segment>> {
        let mut segments = Vec::new();
        let mut current_segment: Option<SegmentBuilder> = None;
        let mut unknown_start: Option<DateTime<Local>> = None;

        for event in events {
            match event.kind {
                EventKind::Start => {
                    // Close any previous segment if exists
                    if let Some(builder) = current_segment.take() {
                        segments.push(builder.build(event.ts));
                    }

                    // Start new segment
                    current_segment = Some(SegmentBuilder {
                        task_key: event.task_key.clone(),
                        start_ts: event.ts,
                        status: SegmentStatus::Confirmed,
                    });
                }
                EventKind::End => {
                    // Close current segment
                    if let Some(builder) = current_segment.take() {
                        segments.push(builder.build(event.ts));
                    }
                }
                EventKind::Pause => {
                    // Already ended by preceding End event
                    // Pause itself doesn't create a segment
                }
                EventKind::Resume => {
                    // Resume just marks state, next Start will create segment
                }
                EventKind::UnknownBegin => {
                    // Close current segment if exists
                    if let Some(builder) = current_segment.take() {
                        segments.push(builder.build(event.ts));
                    }
                    unknown_start = Some(event.ts);
                }
                EventKind::UnknownEnd => {
                    if let Some(start) = unknown_start.take() {
                        segments.push(Segment {
                            id: None,
                            task_key: None,
                            start_ts: start,
                            end_ts: Some(event.ts),
                            minutes: Some(calculate_minutes(start, event.ts)),
                            status: SegmentStatus::Unknown,
                            synced: false,
                        });
                    }
                }
                EventKind::Resolve => {
                    // Resolve events modify existing segments
                    // For now, we'll handle this in a separate pass
                }
                EventKind::Ping => {
                    // Ping events don't affect segments directly
                }
            }
        }

        // If there's still an open segment, keep it open
        if let Some(builder) = current_segment {
            segments.push(Segment {
                id: None,
                task_key: builder.task_key,
                start_ts: builder.start_ts,
                end_ts: None,
                minutes: None,
                status: builder.status,
                synced: false,
            });
        }

        // If there's still an open unknown period
        if let Some(start) = unknown_start {
            segments.push(Segment {
                id: None,
                task_key: None,
                start_ts: start,
                end_ts: None,
                minutes: None,
                status: SegmentStatus::Unknown,
                synced: false,
            });
        }

        Ok(segments)
    }

    /// Rebuild segments table from events
    pub fn rebuild_segments(&self) -> Result<()> {
        let segments = self.derive_all()?;

        self.store.clear_segments()?;

        for segment in segments {
            self.store.insert_segment(&segment)?;
        }

        Ok(())
    }

    /// Get segments summary for a date
    pub fn get_daily_summary(&self, date: chrono::NaiveDate) -> Result<DailySummary> {
        let segments = self.store.get_segments_for_date(date)?;

        let mut task_minutes: HashMap<String, i64> = HashMap::new();
        let mut total_minutes = 0i64;
        let mut unknown_minutes = 0i64;

        for segment in &segments {
            if let Some(minutes) = segment.minutes {
                match &segment.task_key {
                    Some(task_key) => {
                        *task_minutes.entry(task_key.clone()).or_insert(0) += minutes;
                        total_minutes += minutes;
                    }
                    None => {
                        unknown_minutes += minutes;
                    }
                }
            }
        }

        Ok(DailySummary {
            date,
            task_minutes,
            total_minutes,
            unknown_minutes,
            segment_count: segments.len(),
        })
    }
}

struct SegmentBuilder {
    task_key: Option<String>,
    start_ts: DateTime<Local>,
    status: SegmentStatus,
}

impl SegmentBuilder {
    fn build(self, end_ts: DateTime<Local>) -> Segment {
        let minutes = calculate_minutes(self.start_ts, end_ts);
        Segment {
            id: None,
            task_key: self.task_key,
            start_ts: self.start_ts,
            end_ts: Some(end_ts),
            minutes: Some(minutes),
            status: self.status,
            synced: false,
        }
    }
}

fn calculate_minutes(start: DateTime<Local>, end: DateTime<Local>) -> i64 {
    let duration = end.signed_duration_since(start);
    duration.num_seconds() / 60
}

#[derive(Debug)]
pub struct DailySummary {
    pub date: chrono::NaiveDate,
    pub task_minutes: HashMap<String, i64>,
    pub total_minutes: i64,
    pub unknown_minutes: i64,
    pub segment_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{EventSource, Store};
    use chrono::TimeZone;
    use uuid::Uuid;

    fn make_event(kind: EventKind, task_key: Option<&str>, ts: DateTime<Local>) -> Event {
        Event {
            id: None,
            uuid: Uuid::new_v4(),
            ts,
            mono_ms: 0,
            boot_id: "test".to_string(),
            kind,
            task_key: task_key.map(|s| s.to_string()),
            ref_event_id: None,
            note: None,
            source: EventSource::User,
            synced: false,
        }
    }

    #[test]
    fn test_single_task_segment() {
        let store = Store::new_in_memory().unwrap();
        let deriver = SegmentDeriver::new(store);

        let t1 = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();
        let t2 = Local.with_ymd_and_hms(2026, 10, 21, 9, 25, 0).unwrap();

        let events = vec![
            make_event(EventKind::Start, Some("Task1"), t1),
            make_event(EventKind::End, Some("Task1"), t2),
        ];

        let segments = deriver.derive_from_events(&events).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].task_key, Some("Task1".to_string()));
        assert_eq!(segments[0].minutes, Some(25));
        assert_eq!(segments[0].status, SegmentStatus::Confirmed);
    }

    #[test]
    fn test_task_switch() {
        let store = Store::new_in_memory().unwrap();
        let deriver = SegmentDeriver::new(store);

        let t1 = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();
        let t2 = Local.with_ymd_and_hms(2026, 10, 21, 9, 25, 0).unwrap();
        let t3 = Local.with_ymd_and_hms(2026, 10, 21, 10, 0, 0).unwrap();

        let events = vec![
            make_event(EventKind::Start, Some("Task1"), t1),
            make_event(EventKind::End, Some("Task1"), t2),
            make_event(EventKind::Start, Some("Task2"), t2),
            make_event(EventKind::End, Some("Task2"), t3),
        ];

        let segments = deriver.derive_from_events(&events).unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].task_key, Some("Task1".to_string()));
        assert_eq!(segments[0].minutes, Some(25));
        assert_eq!(segments[1].task_key, Some("Task2".to_string()));
        assert_eq!(segments[1].minutes, Some(35));
        // Check no gap between segments
        assert_eq!(segments[0].end_ts, Some(t2));
        assert_eq!(segments[1].start_ts, t2);
    }

    #[test]
    fn test_unknown_period() {
        let store = Store::new_in_memory().unwrap();
        let deriver = SegmentDeriver::new(store);

        let t1 = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();
        let t2 = Local.with_ymd_and_hms(2026, 10, 21, 9, 25, 0).unwrap();
        let t3 = Local.with_ymd_and_hms(2026, 10, 21, 10, 5, 0).unwrap();

        let events = vec![
            make_event(EventKind::Start, Some("Task1"), t1),
            make_event(EventKind::UnknownBegin, None, t2),
            make_event(EventKind::UnknownEnd, None, t3),
        ];

        let segments = deriver.derive_from_events(&events).unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].task_key, Some("Task1".to_string()));
        assert_eq!(segments[0].minutes, Some(25));
        assert_eq!(segments[1].task_key, None);
        assert_eq!(segments[1].minutes, Some(40));
        assert_eq!(segments[1].status, SegmentStatus::Unknown);
    }

    #[test]
    fn test_open_segment() {
        let store = Store::new_in_memory().unwrap();
        let deriver = SegmentDeriver::new(store);

        let t1 = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();

        let events = vec![make_event(EventKind::Start, Some("Task1"), t1)];

        let segments = deriver.derive_from_events(&events).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].task_key, Some("Task1".to_string()));
        assert_eq!(segments[0].end_ts, None);
        assert_eq!(segments[0].minutes, None);
    }

    #[test]
    fn test_pause_and_resume() {
        let store = Store::new_in_memory().unwrap();
        let deriver = SegmentDeriver::new(store);

        let t1 = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();
        let t2 = Local.with_ymd_and_hms(2026, 10, 21, 9, 25, 0).unwrap();
        let t3 = Local.with_ymd_and_hms(2026, 10, 21, 9, 50, 0).unwrap();

        let events = vec![
            make_event(EventKind::Start, Some("Task1"), t1),
            make_event(EventKind::End, Some("Task1"), t2),
            make_event(EventKind::Pause, None, t2),
            make_event(EventKind::Resume, None, t3),
            make_event(EventKind::Start, Some("Task2"), t3),
        ];

        let segments = deriver.derive_from_events(&events).unwrap();
        // Should have Task1 segment, but no segment for pause period
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].task_key, Some("Task1".to_string()));
        assert_eq!(segments[0].minutes, Some(25));
        assert_eq!(segments[1].task_key, Some("Task2".to_string()));
    }

    #[test]
    fn test_rebuild_segments() {
        let store = Store::new_in_memory().unwrap();
        let deriver = SegmentDeriver::new(store.clone());

        let t1 = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();
        let t2 = Local.with_ymd_and_hms(2026, 10, 21, 9, 25, 0).unwrap();

        let event1 = make_event(EventKind::Start, Some("Task1"), t1);
        let event2 = make_event(EventKind::End, Some("Task1"), t2);

        store.insert_event(&event1).unwrap();
        store.insert_event(&event2).unwrap();

        deriver.rebuild_segments().unwrap();

        let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let segments = store.get_segments_for_date(date).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].minutes, Some(25));
    }
}
