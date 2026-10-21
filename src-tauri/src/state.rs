use crate::store::{Event, EventKind, EventSource};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppState {
    Idle,
    Working { task_key: String },
    Paused,
}

#[derive(Clone)]
pub struct StateMachine {
    state: Arc<Mutex<AppState>>,
    boot_id: String,
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AppState::Idle)),
            boot_id: Uuid::new_v4().to_string(),
        }
    }

    pub fn get_state(&self) -> AppState {
        self.state.lock().unwrap().clone()
    }

    pub fn get_boot_id(&self) -> String {
        self.boot_id.clone()
    }

    /// Start working on a task
    pub fn start_task(
        &self,
        task_key: String,
        ts: DateTime<Local>,
        mono_ms: i64,
        source: EventSource,
        note: Option<String>,
    ) -> Vec<Event> {
        let mut state = self.state.lock().unwrap();
        let mut events = Vec::new();

        match &*state {
            AppState::Idle => {
                events.push(Event {
                    id: None,
                    uuid: Uuid::new_v4(),
                    ts,
                    mono_ms,
                    boot_id: self.boot_id.clone(),
                    kind: EventKind::Start,
                    task_key: Some(task_key.clone()),
                    ref_event_id: None,
                    note,
                    source,
                    synced: false,
                });
                *state = AppState::Working { task_key };
            }
            AppState::Working {
                task_key: current_task,
            } => {
                if current_task != &task_key {
                    // Switch: end current task and start new task at same timestamp
                    events.push(Event {
                        id: None,
                        uuid: Uuid::new_v4(),
                        ts,
                        mono_ms,
                        boot_id: self.boot_id.clone(),
                        kind: EventKind::End,
                        task_key: Some(current_task.clone()),
                        ref_event_id: None,
                        note: None,
                        source: source.clone(),
                        synced: false,
                    });
                    events.push(Event {
                        id: None,
                        uuid: Uuid::new_v4(),
                        ts,
                        mono_ms,
                        boot_id: self.boot_id.clone(),
                        kind: EventKind::Start,
                        task_key: Some(task_key.clone()),
                        ref_event_id: None,
                        note,
                        source,
                        synced: false,
                    });
                    *state = AppState::Working { task_key };
                }
                // If same task, just continue (only ping event will be recorded separately)
            }
            AppState::Paused => {
                events.push(Event {
                    id: None,
                    uuid: Uuid::new_v4(),
                    ts,
                    mono_ms,
                    boot_id: self.boot_id.clone(),
                    kind: EventKind::Resume,
                    task_key: None,
                    ref_event_id: None,
                    note: None,
                    source: source.clone(),
                    synced: false,
                });
                events.push(Event {
                    id: None,
                    uuid: Uuid::new_v4(),
                    ts,
                    mono_ms,
                    boot_id: self.boot_id.clone(),
                    kind: EventKind::Start,
                    task_key: Some(task_key.clone()),
                    ref_event_id: None,
                    note,
                    source,
                    synced: false,
                });
                *state = AppState::Working { task_key };
            }
        }

        events
    }

    /// End current task
    pub fn end_task(
        &self,
        ts: DateTime<Local>,
        mono_ms: i64,
        source: EventSource,
    ) -> Vec<Event> {
        let mut state = self.state.lock().unwrap();
        let mut events = Vec::new();

        match &*state {
            AppState::Working { task_key } => {
                events.push(Event {
                    id: None,
                    uuid: Uuid::new_v4(),
                    ts,
                    mono_ms,
                    boot_id: self.boot_id.clone(),
                    kind: EventKind::End,
                    task_key: Some(task_key.clone()),
                    ref_event_id: None,
                    note: None,
                    source,
                    synced: false,
                });
                *state = AppState::Idle;
            }
            _ => {
                // No-op if not working
            }
        }

        events
    }

    /// Pause current work
    pub fn pause(&self, ts: DateTime<Local>, mono_ms: i64, source: EventSource) -> Vec<Event> {
        let mut state = self.state.lock().unwrap();
        let mut events = Vec::new();

        match &*state {
            AppState::Working { task_key } => {
                events.push(Event {
                    id: None,
                    uuid: Uuid::new_v4(),
                    ts,
                    mono_ms,
                    boot_id: self.boot_id.clone(),
                    kind: EventKind::End,
                    task_key: Some(task_key.clone()),
                    ref_event_id: None,
                    note: None,
                    source: source.clone(),
                    synced: false,
                });
                events.push(Event {
                    id: None,
                    uuid: Uuid::new_v4(),
                    ts,
                    mono_ms,
                    boot_id: self.boot_id.clone(),
                    kind: EventKind::Pause,
                    task_key: None,
                    ref_event_id: None,
                    note: None,
                    source,
                    synced: false,
                });
                *state = AppState::Paused;
            }
            _ => {
                // Already paused or idle
            }
        }

        events
    }

    /// Record a ping event (doesn't change state)
    pub fn ping(&self, ts: DateTime<Local>, mono_ms: i64) -> Event {
        Event {
            id: None,
            uuid: Uuid::new_v4(),
            ts,
            mono_ms,
            boot_id: self.boot_id.clone(),
            kind: EventKind::Ping,
            task_key: None,
            ref_event_id: None,
            note: None,
            source: EventSource::Ping,
            synced: false,
        }
    }

    /// Mark an unknown period
    pub fn mark_unknown_begin(
        &self,
        ts: DateTime<Local>,
        mono_ms: i64,
        source: EventSource,
    ) -> Event {
        Event {
            id: None,
            uuid: Uuid::new_v4(),
            ts,
            mono_ms,
            boot_id: self.boot_id.clone(),
            kind: EventKind::UnknownBegin,
            task_key: None,
            ref_event_id: None,
            note: None,
            source,
            synced: false,
        }
    }

    pub fn mark_unknown_end(
        &self,
        ts: DateTime<Local>,
        mono_ms: i64,
        source: EventSource,
    ) -> Event {
        Event {
            id: None,
            uuid: Uuid::new_v4(),
            ts,
            mono_ms,
            boot_id: self.boot_id.clone(),
            kind: EventKind::UnknownEnd,
            task_key: None,
            ref_event_id: None,
            note: None,
            source,
            synced: false,
        }
    }

    /// Restore state from last known event
    pub fn restore_from_event(&self, event: &Event) {
        let mut state = self.state.lock().unwrap();
        *state = match event.kind {
            EventKind::Start => AppState::Working {
                task_key: event.task_key.clone().unwrap_or_default(),
            },
            EventKind::Pause => AppState::Paused,
            EventKind::End => AppState::Idle,
            _ => AppState::Idle,
        };
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_time() -> DateTime<Local> {
        Local::now()
    }

    #[test]
    fn test_start_from_idle() {
        let sm = StateMachine::new();
        let events = sm.start_task(
            "Task1".to_string(),
            test_time(),
            1000,
            EventSource::User,
            None,
        );

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::Start);
        assert_eq!(sm.get_state(), AppState::Working {
            task_key: "Task1".to_string()
        });
    }

    #[test]
    fn test_switch_task() {
        let sm = StateMachine::new();
        sm.start_task(
            "Task1".to_string(),
            test_time(),
            1000,
            EventSource::User,
            None,
        );

        let ts = test_time();
        let events = sm.start_task("Task2".to_string(), ts, 2000, EventSource::User, None);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventKind::End);
        assert_eq!(events[0].task_key, Some("Task1".to_string()));
        assert_eq!(events[1].kind, EventKind::Start);
        assert_eq!(events[1].task_key, Some("Task2".to_string()));
        // Both events should have same timestamp
        assert_eq!(events[0].ts, events[1].ts);
        assert_eq!(sm.get_state(), AppState::Working {
            task_key: "Task2".to_string()
        });
    }

    #[test]
    fn test_continue_same_task() {
        let sm = StateMachine::new();
        sm.start_task(
            "Task1".to_string(),
            test_time(),
            1000,
            EventSource::User,
            None,
        );

        let events = sm.start_task(
            "Task1".to_string(),
            test_time(),
            2000,
            EventSource::User,
            None,
        );

        assert_eq!(events.len(), 0); // No new events when continuing same task
    }

    #[test]
    fn test_pause_and_resume() {
        let sm = StateMachine::new();
        sm.start_task(
            "Task1".to_string(),
            test_time(),
            1000,
            EventSource::User,
            None,
        );

        let events = sm.pause(test_time(), 2000, EventSource::User);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventKind::End);
        assert_eq!(events[1].kind, EventKind::Pause);
        assert_eq!(sm.get_state(), AppState::Paused);

        let events = sm.start_task(
            "Task2".to_string(),
            test_time(),
            3000,
            EventSource::User,
            None,
        );
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventKind::Resume);
        assert_eq!(events[1].kind, EventKind::Start);
    }

    #[test]
    fn test_end_task() {
        let sm = StateMachine::new();
        sm.start_task(
            "Task1".to_string(),
            test_time(),
            1000,
            EventSource::User,
            None,
        );

        let events = sm.end_task(test_time(), 2000, EventSource::Auto);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::End);
        assert_eq!(sm.get_state(), AppState::Idle);
    }
}
