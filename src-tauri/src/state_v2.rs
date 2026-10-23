use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::models::{SlotType, TimeSlot};
use crate::store_v2::StoreV2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AppStateEnum {
    Idle,
    Working,
    Interrupt,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateData {
    #[serde(rename = "status")]
    pub state: AppStateEnum,
    pub current_slot_id: Option<i64>,
    pub paused_slot_id: Option<i64>,
    pub remaining_seconds: Option<i64>,
    pub pomodoro_start: Option<DateTime<Local>>,
}

impl Default for StateData {
    fn default() -> Self {
        Self {
            state: AppStateEnum::Idle,
            current_slot_id: None,
            paused_slot_id: None,
            remaining_seconds: None,
            pomodoro_start: None,
        }
    }
}

pub struct StateManager {
    store: Arc<StoreV2>,
    state: Arc<Mutex<StateData>>,
}

impl StateManager {
    pub fn new(store: Arc<StoreV2>) -> Self {
        Self {
            store,
            state: Arc::new(Mutex::new(StateData::default())),
        }
    }

    pub fn get_state(&self) -> StateData {
        self.state.lock().unwrap().clone()
    }

    pub fn start_task(&self, task_name: String, slot_type: SlotType) -> anyhow::Result<i64> {
        let mut state = self.state.lock().unwrap();

        if state.current_slot_id.is_some() {
            anyhow::bail!("Already have an active task");
        }

        let now = Local::now();
        let slot = TimeSlot::new(task_name.clone(), slot_type.clone(), now);
        let id = self.store.create_slot(&slot)?;
        self.store.add_or_update_recent_task(&task_name, slot_type)?;

        state.state = AppStateEnum::Working;
        state.current_slot_id = Some(id);
        state.pomodoro_start = Some(now);
        state.remaining_seconds = None;

        Ok(id)
    }

    pub fn end_task(&self, note: Option<String>) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();

        if let Some(slot_id) = state.current_slot_id {
            self.store.update_slot(slot_id, Some(Local::now()), note)?;
            state.state = AppStateEnum::Idle;
            state.current_slot_id = None;
            state.pomodoro_start = None;
            state.remaining_seconds = None;
        } else {
            anyhow::bail!("No active task to end");
        }

        Ok(())
    }

    /// Close the active or paused slot at a specific time.
    pub fn end_task_at(&self, end_ts: DateTime<Local>) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        let slot_id = state.current_slot_id.or(state.paused_slot_id);

        if let Some(slot_id) = slot_id {
            self.store.update_slot(slot_id, Some(end_ts), None)?;
            state.state = AppStateEnum::Idle;
            state.current_slot_id = None;
            state.paused_slot_id = None;
            state.pomodoro_start = None;
            state.remaining_seconds = None;
        } else {
            anyhow::bail!("No active task to end");
        }

        Ok(())
    }

    pub fn pause_task(&self) -> anyhow::Result<i64> {
        let mut state = self.state.lock().unwrap();

        if let Some(slot_id) = state.current_slot_id {
            if let Some(start) = state.pomodoro_start {
                let elapsed = Local::now().signed_duration_since(start).num_seconds();
                let pomodoro_secs = self.store.get_settings()?.pomodoro_minutes * 60;
                state.remaining_seconds = Some((pomodoro_secs - elapsed).max(0));
            }

            state.state = AppStateEnum::Paused;
            state.paused_slot_id = Some(slot_id);
            Ok(slot_id)
        } else {
            anyhow::bail!("No active task to pause");
        }
    }

    pub fn resume_task(&self) -> anyhow::Result<i64> {
        let mut state = self.state.lock().unwrap();

        if let Some(slot_id) = state.paused_slot_id {
            state.state = AppStateEnum::Working;
            state.current_slot_id = Some(slot_id);
            state.paused_slot_id = None;
            
            if state.remaining_seconds.is_some() {
                state.pomodoro_start = Some(Local::now());
            }

            Ok(slot_id)
        } else {
            anyhow::bail!("No paused task to resume");
        }
    }

    pub fn start_interrupt(&self, who: Option<String>, reason: Option<String>) -> anyhow::Result<i64> {
        let mut state = self.state.lock().unwrap();

        let parent_id = if let Some(slot_id) = state.current_slot_id {
            let elapsed = if let Some(start) = state.pomodoro_start {
                Local::now().signed_duration_since(start).num_seconds()
            } else {
                0
            };
            let pomodoro_secs = self.store.get_settings()?.pomodoro_minutes * 60;
            state.remaining_seconds = Some((pomodoro_secs - elapsed).max(0));

            state.paused_slot_id = Some(slot_id);
            Some(slot_id)
        } else {
            None
        };

        let task_name = format!("插隊：{}", who.as_deref().unwrap_or("未註明"));
        let mut slot = TimeSlot::new(task_name, SlotType::Interrupt, Local::now());
        slot.parent_id = parent_id;
        slot.note = reason;
        
        let interrupt_id = self.store.create_slot(&slot)?;

        if let Some(name) = who {
            self.store.add_or_update_recent_person(&name)?;
        }

        state.state = AppStateEnum::Interrupt;
        state.current_slot_id = Some(interrupt_id);

        Ok(interrupt_id)
    }

    pub fn end_interrupt_and_resume(&self) -> anyhow::Result<Option<i64>> {
        let mut state = self.state.lock().unwrap();

        if state.state != AppStateEnum::Interrupt {
            anyhow::bail!("Not in interrupt state");
        }

        if let Some(interrupt_id) = state.current_slot_id {
            self.store.update_slot(interrupt_id, Some(Local::now()), None)?;
        }

        if let Some(paused_id) = state.paused_slot_id {
            state.state = AppStateEnum::Working;
            state.current_slot_id = Some(paused_id);
            state.paused_slot_id = None;
            
            if state.remaining_seconds.is_some() {
                state.pomodoro_start = Some(Local::now());
            }

            Ok(Some(paused_id))
        } else {
            state.state = AppStateEnum::Idle;
            state.current_slot_id = None;
            Ok(None)
        }
    }

    pub fn get_elapsed_seconds(&self) -> Option<i64> {
        let state = self.state.lock().unwrap();
        state.pomodoro_start.map(|start| {
            Local::now().signed_duration_since(start).num_seconds()
        })
    }

    pub fn get_remaining_seconds(&self) -> Option<i64> {
        let state = self.state.lock().unwrap();
        let pomodoro_secs = self.store.get_settings().ok()?.pomodoro_minutes * 60;

        match state.state {
            AppStateEnum::Working => {
                if let Some(start) = state.pomodoro_start {
                    let elapsed = Local::now().signed_duration_since(start).num_seconds();
                    Some((pomodoro_secs - elapsed).max(0))
                } else {
                    None
                }
            }
            AppStateEnum::Paused | AppStateEnum::Interrupt => state.remaining_seconds,
            AppStateEnum::Idle => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_start_and_end_task() {
        let store = Arc::new(StoreV2::new(Path::new(":memory:")).unwrap());
        let mgr = StateManager::new(store.clone());

        let id = mgr.start_task("Test Task".to_string(), SlotType::Work).unwrap();
        assert_eq!(mgr.get_state().state, AppStateEnum::Working);
        assert_eq!(mgr.get_state().current_slot_id, Some(id));

        mgr.end_task(None).unwrap();
        assert_eq!(mgr.get_state().state, AppStateEnum::Idle);
        assert_eq!(mgr.get_state().current_slot_id, None);

        let slot = store.get_slot(id).unwrap().unwrap();
        assert!(slot.end_ts.is_some());
    }

    #[test]
    fn test_pause_and_resume() {
        let store = Arc::new(StoreV2::new(Path::new(":memory:")).unwrap());
        let mgr = StateManager::new(store.clone());

        let id = mgr.start_task("Test Task".to_string(), SlotType::Work).unwrap();
        mgr.pause_task().unwrap();
        
        assert_eq!(mgr.get_state().state, AppStateEnum::Paused);
        assert_eq!(mgr.get_state().paused_slot_id, Some(id));

        mgr.resume_task().unwrap();
        assert_eq!(mgr.get_state().state, AppStateEnum::Working);
        assert_eq!(mgr.get_state().current_slot_id, Some(id));
    }

    #[test]
    fn test_interrupt_flow() {
        let store = Arc::new(StoreV2::new(Path::new(":memory:")).unwrap());
        let mgr = StateManager::new(store.clone());

        let work_id = mgr.start_task("Work Task".to_string(), SlotType::Work).unwrap();
        let interrupt_id = mgr.start_interrupt(Some("Kelly".to_string()), Some("緊急事項".to_string())).unwrap();

        assert_eq!(mgr.get_state().state, AppStateEnum::Interrupt);
        assert_eq!(mgr.get_state().current_slot_id, Some(interrupt_id));
        assert_eq!(mgr.get_state().paused_slot_id, Some(work_id));

        let resumed_id = mgr.end_interrupt_and_resume().unwrap();
        assert_eq!(resumed_id, Some(work_id));
        assert_eq!(mgr.get_state().state, AppStateEnum::Working);
        assert_eq!(mgr.get_state().current_slot_id, Some(work_id));

        let interrupt_slot = store.get_slot(interrupt_id).unwrap().unwrap();
        assert_eq!(interrupt_slot.parent_id, Some(work_id));
        assert!(interrupt_slot.end_ts.is_some());
    }
}
