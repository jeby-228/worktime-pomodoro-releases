use crate::state::StateMachine;
use crate::store::{Event, EventKind, EventSource, Store};
use anyhow::Result;
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone};
use std::time::Instant;

pub struct Recovery {
    store: Store,
    state_machine: StateMachine,
    start_instant: Instant,
}

#[derive(Debug)]
pub struct RecoveryAction {
    pub events_to_insert: Vec<Event>,
    pub message: String,
    pub needs_user_resolve: bool,
}

impl Recovery {
    pub fn new(store: Store, state_machine: StateMachine) -> Self {
        Self {
            store,
            state_machine,
            start_instant: Instant::now(),
        }
    }

    /// Check and perform recovery on startup
    pub fn check_on_startup(&self, now: DateTime<Local>) -> Result<Option<RecoveryAction>> {
        let last_event = self.store.get_last_event()?;

        let last_event = match last_event {
            Some(e) => e,
            None => {
                // No events yet, nothing to recover
                return Ok(None);
            }
        };

        // Check 1: Is the boot_id different? (New session)
        if last_event.boot_id == self.state_machine.get_boot_id() {
            // Same session, no recovery needed
            return Ok(None);
        }

        // Check 2: Did we cross a date boundary?
        let last_date = last_event.ts.date_naive();
        let current_date = now.date_naive();

        if last_date != current_date {
            return self.recover_from_overnight(last_event, last_date, now);
        }

        // Check 3: Was the app working when it crashed?
        if matches!(last_event.kind, EventKind::Start) {
            return self.recover_from_crash(last_event, now);
        }

        // Check 4: Check for time jump (system clock changed)
        // This would be detected by comparing mono_ms with actual time difference
        // For now, we'll implement a basic version

        Ok(None)
    }

    /// Recover from crash while working
    fn recover_from_crash(
        &self,
        last_start_event: Event,
        now: DateTime<Local>,
    ) -> Result<Option<RecoveryAction>> {
        // Find the last ping event after this start
        let events_since = self.store.get_events_since(last_start_event.ts)?;
        
        let last_ping = events_since
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Ping))
            .last();

        let recovery_end_time = if let Some(ping) = last_ping {
            // End at last ping time
            ping.ts
        } else {
            // No ping, end at the start time (conservative)
            last_start_event.ts
        };

        let mono_ms = self.elapsed_mono_ms();

        let mut events = Vec::new();

        // End the task at recovery_end_time
        events.push(Event {
            id: None,
            uuid: uuid::Uuid::new_v4(),
            ts: recovery_end_time,
            mono_ms,
            boot_id: self.state_machine.get_boot_id(),
            kind: EventKind::End,
            task_key: last_start_event.task_key.clone(),
            ref_event_id: None,
            note: Some("程式異常結束時自動恢復".to_string()),
            source: EventSource::Recovery,
            synced: false,
        });

        // Mark unknown period from recovery_end_time to now
        if now.signed_duration_since(recovery_end_time) > Duration::minutes(1) {
            events.push(Event {
                id: None,
                uuid: uuid::Uuid::new_v4(),
                ts: recovery_end_time,
                mono_ms,
                boot_id: self.state_machine.get_boot_id(),
                kind: EventKind::UnknownBegin,
                task_key: None,
                ref_event_id: None,
                note: None,
                source: EventSource::Recovery,
                synced: false,
            });

            events.push(Event {
                id: None,
                uuid: uuid::Uuid::new_v4(),
                ts: now,
                mono_ms,
                boot_id: self.state_machine.get_boot_id(),
                kind: EventKind::UnknownEnd,
                task_key: None,
                ref_event_id: None,
                note: None,
                source: EventSource::Recovery,
                synced: false,
            });
        }

        let message = format!(
            "偵測到程式異常結束。已將任務「{}」結束於 {}，之後的時間標記為待確認。",
            last_start_event.task_key.as_deref().unwrap_or("未知"),
            recovery_end_time.format("%H:%M")
        );

        Ok(Some(RecoveryAction {
            events_to_insert: events,
            message,
            needs_user_resolve: true,
        }))
    }

    /// Recover from overnight shutdown
    fn recover_from_overnight(
        &self,
        last_event: Event,
        last_date: NaiveDate,
        now: DateTime<Local>,
    ) -> Result<Option<RecoveryAction>> {
        let mut events = Vec::new();
        let mono_ms = self.elapsed_mono_ms();

        // If the last event was a Start, we need to end it
        if matches!(last_event.kind, EventKind::Start) {
            // Find last ping or use default end-of-workday time
            let events_on_last_day = self.store.get_events_since(
                last_date.and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_local_timezone(Local)
                    .unwrap()
            )?;

            let last_ping = events_on_last_day
                .iter()
                .filter(|e| matches!(e.kind, EventKind::Ping))
                .last();

            let end_time = if let Some(ping) = last_ping {
                ping.ts
            } else {
                // Default to 17:30 on that day
                last_date
                    .and_hms_opt(17, 30, 0)
                    .unwrap()
                    .and_local_timezone(Local)
                    .unwrap()
            };

            events.push(Event {
                id: None,
                uuid: uuid::Uuid::new_v4(),
                ts: end_time,
                mono_ms,
                boot_id: self.state_machine.get_boot_id(),
                kind: EventKind::End,
                task_key: last_event.task_key.clone(),
                ref_event_id: None,
                note: Some("跨日自動結束".to_string()),
                source: EventSource::Recovery,
                synced: false,
            });
        }

        let message = format!(
            "偵測到跨日啟動。已將前一日 ({}) 的工作結束。",
            last_date.format("%m/%d")
        );

        Ok(Some(RecoveryAction {
            events_to_insert: events,
            message,
            needs_user_resolve: false,
        }))
    }

    /// Handle sleep/wake cycle
    pub fn handle_sleep(
        &self,
        sleep_start: DateTime<Local>,
    ) -> Result<Vec<Event>> {
        let mono_ms = self.elapsed_mono_ms();
        let mut events = Vec::new();

        // End current work if any
        let state = self.state_machine.get_state();
        if let crate::state::AppState::Working { task_key } = state {
            events.push(Event {
                id: None,
                uuid: uuid::Uuid::new_v4(),
                ts: sleep_start,
                mono_ms,
                boot_id: self.state_machine.get_boot_id(),
                kind: EventKind::End,
                task_key: Some(task_key),
                ref_event_id: None,
                note: None,
                source: EventSource::Auto,
                synced: false,
            });
        }

        // Mark unknown begin
        events.push(Event {
            id: None,
            uuid: uuid::Uuid::new_v4(),
            ts: sleep_start,
            mono_ms,
            boot_id: self.state_machine.get_boot_id(),
            kind: EventKind::UnknownBegin,
            task_key: None,
            ref_event_id: None,
            note: Some("電腦睡眠".to_string()),
            source: EventSource::Auto,
            synced: false,
        });

        Ok(events)
    }

    pub fn handle_wake(
        &self,
        wake_time: DateTime<Local>,
    ) -> Result<Vec<Event>> {
        let mono_ms = self.elapsed_mono_ms();
        let mut events = Vec::new();

        // Mark unknown end
        events.push(Event {
            id: None,
            uuid: uuid::Uuid::new_v4(),
            ts: wake_time,
            mono_ms,
            boot_id: self.state_machine.get_boot_id(),
            kind: EventKind::UnknownEnd,
            task_key: None,
            ref_event_id: None,
            note: Some("電腦喚醒".to_string()),
            source: EventSource::Auto,
            synced: false,
        });

        Ok(events)
    }

    /// Detect time jump (system clock changed)
    pub fn detect_time_jump(
        &self,
        expected_mono_diff_ms: i64,
        actual_ts_diff_secs: i64,
    ) -> bool {
        let expected_secs = expected_mono_diff_ms / 1000;
        let diff = (actual_ts_diff_secs - expected_secs).abs();
        
        // If difference is more than 2 minutes, consider it a time jump
        diff > 120
    }

    fn elapsed_mono_ms(&self) -> i64 {
        self.start_instant.elapsed().as_millis() as i64
    }

    /// Apply recovery action
    pub fn apply_recovery(&self, action: RecoveryAction) -> Result<()> {
        for event in action.events_to_insert {
            self.store.insert_event(&event)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::StateMachine;
    use crate::store::{EventKind, Store};
    use chrono::TimeZone;
    use uuid::Uuid;

    #[test]
    fn test_recover_from_crash() {
        let store = Store::new_in_memory().unwrap();
        let sm = StateMachine::new();
        
        // Simulate a crash: last event was a start
        let crash_time = Local.with_ymd_and_hms(2026, 10, 21, 10, 0, 0).unwrap();
        let ping_time = Local.with_ymd_and_hms(2026, 10, 21, 10, 25, 0).unwrap();
        
        let start_event = Event {
            id: None,
            uuid: Uuid::new_v4(),
            ts: crash_time,
            mono_ms: 0,
            boot_id: "old-boot".to_string(),
            kind: EventKind::Start,
            task_key: Some("Task1".to_string()),
            ref_event_id: None,
            note: None,
            source: EventSource::User,
            synced: false,
        };
        
        let ping_event = Event {
            id: None,
            uuid: Uuid::new_v4(),
            ts: ping_time,
            mono_ms: 1000,
            boot_id: "old-boot".to_string(),
            kind: EventKind::Ping,
            task_key: None,
            ref_event_id: None,
            note: None,
            source: EventSource::Ping,
            synced: false,
        };
        
        store.insert_event(&start_event).unwrap();
        store.insert_event(&ping_event).unwrap();

        let recovery = Recovery::new(store, sm);
        let now = Local.with_ymd_and_hms(2026, 10, 21, 11, 0, 0).unwrap();
        
        let action = recovery.check_on_startup(now).unwrap();
        assert!(action.is_some());
        
        let action = action.unwrap();
        assert!(action.needs_user_resolve);
        assert!(!action.events_to_insert.is_empty());
        
        // Should have end event at ping time
        let end_event = action.events_to_insert.iter()
            .find(|e| matches!(e.kind, EventKind::End))
            .unwrap();
        assert_eq!(end_event.ts, ping_time);
    }

    #[test]
    fn test_recover_from_overnight() {
        let store = Store::new_in_memory().unwrap();
        let sm = StateMachine::new();
        
        let yesterday = Local.with_ymd_and_hms(2026, 10, 20, 16, 0, 0).unwrap();
        let yesterday_ping = Local.with_ymd_and_hms(2026, 10, 20, 16, 25, 0).unwrap();
        
        let start_event = Event {
            id: None,
            uuid: Uuid::new_v4(),
            ts: yesterday,
            mono_ms: 0,
            boot_id: "old-boot".to_string(),
            kind: EventKind::Start,
            task_key: Some("Task1".to_string()),
            ref_event_id: None,
            note: None,
            source: EventSource::User,
            synced: false,
        };
        
        let ping_event = Event {
            id: None,
            uuid: Uuid::new_v4(),
            ts: yesterday_ping,
            mono_ms: 1000,
            boot_id: "old-boot".to_string(),
            kind: EventKind::Ping,
            task_key: None,
            ref_event_id: None,
            note: None,
            source: EventSource::Ping,
            synced: false,
        };
        
        store.insert_event(&start_event).unwrap();
        store.insert_event(&ping_event).unwrap();

        let recovery = Recovery::new(store, sm);
        let today = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();
        
        let action = recovery.check_on_startup(today).unwrap();
        assert!(action.is_some());
        
        let action = action.unwrap();
        assert!(!action.needs_user_resolve);
        
        // Should have end event at yesterday's ping time
        let end_event = &action.events_to_insert[0];
        assert!(matches!(end_event.kind, EventKind::End));
        assert_eq!(end_event.ts, yesterday_ping);
    }

    #[test]
    fn test_handle_sleep_wake() {
        let store = Store::new_in_memory().unwrap();
        let sm = StateMachine::new();
        
        // Start working
        let work_start = Local.with_ymd_and_hms(2026, 10, 21, 10, 0, 0).unwrap();
        let start_events = sm.start_task(
            "Task1".to_string(),
            work_start,
            0,
            EventSource::User,
            None,
        );
        for event in start_events {
            store.insert_event(&event).unwrap();
        }

        let recovery = Recovery::new(store.clone(), sm);
        
        let sleep_time = Local.with_ymd_and_hms(2026, 10, 21, 10, 30, 0).unwrap();
        let sleep_events = recovery.handle_sleep(sleep_time).unwrap();
        assert_eq!(sleep_events.len(), 2); // End + UnknownBegin
        assert!(matches!(sleep_events[0].kind, EventKind::End));
        assert!(matches!(sleep_events[1].kind, EventKind::UnknownBegin));

        let wake_time = Local.with_ymd_and_hms(2026, 10, 21, 11, 0, 0).unwrap();
        let wake_events = recovery.handle_wake(wake_time).unwrap();
        assert_eq!(wake_events.len(), 1); // UnknownEnd
        assert!(matches!(wake_events[0].kind, EventKind::UnknownEnd));
    }

    #[test]
    fn test_detect_time_jump() {
        let store = Store::new_in_memory().unwrap();
        let sm = StateMachine::new();
        let recovery = Recovery::new(store, sm);

        // Normal case: 5 minutes passed
        assert!(!recovery.detect_time_jump(5 * 60 * 1000, 5 * 60));

        // Time jump: mono says 5 minutes, but clock jumped 1 hour
        assert!(recovery.detect_time_jump(5 * 60 * 1000, 60 * 60));

        // Small drift is OK (within 2 minutes)
        assert!(!recovery.detect_time_jump(5 * 60 * 1000, 5 * 60 + 100));
    }
}
