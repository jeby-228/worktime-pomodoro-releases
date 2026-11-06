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
    /// Work seconds already counted before the current segment.
    /// The interrupt stopwatch ignores this and starts from `pomodoro_start`.
    pub carried_seconds: i64,
    /// The interrupted task was paused, so returning must leave it paused.
    #[serde(default)]
    pub return_to_paused: bool,
}

impl Default for StateData {
    fn default() -> Self {
        Self {
            state: AppStateEnum::Idle,
            current_slot_id: None,
            paused_slot_id: None,
            remaining_seconds: None,
            pomodoro_start: None,
            carried_seconds: 0,
            return_to_paused: false,
        }
    }
}

fn segment_secs(state: &StateData) -> i64 {
    state
        .pomodoro_start
        .map(|start| {
            Local::now()
                .signed_duration_since(start)
                .num_seconds()
                .max(0)
        })
        .unwrap_or(0)
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

    pub fn start_task(
        &self,
        task_name: String,
        slot_type: SlotType,
        category_id: Option<i64>,
    ) -> anyhow::Result<i64> {
        let mut state = self.state.lock().unwrap();

        if state.current_slot_id.is_some() {
            anyhow::bail!("Already have an active task");
        }

        let now = Local::now();
        let mut slot = TimeSlot::new(task_name.clone(), slot_type.clone(), now);
        slot.category_id = category_id;
        let id = self.store.create_slot(&slot)?;
        self.store
            .add_or_update_recent_task(&task_name, slot_type)?;

        state.state = AppStateEnum::Working;
        state.current_slot_id = Some(id);
        state.pomodoro_start = Some(now);
        state.remaining_seconds = None;
        state.carried_seconds = 0;

        Ok(id)
    }

    fn clear_activity(state: &mut StateData) {
        state.state = AppStateEnum::Idle;
        state.current_slot_id = None;
        state.paused_slot_id = None;
        state.pomodoro_start = None;
        state.remaining_seconds = None;
        state.carried_seconds = 0;
        state.return_to_paused = false;
    }

    /// End the running slot and start a new one in `category_id` from now.
    /// The task, status and interrupt parent carry over.
    pub fn switch_category(&self, category_id: Option<i64>) -> anyhow::Result<i64> {
        let mut state = self.state.lock().unwrap();

        if !matches!(state.state, AppStateEnum::Working | AppStateEnum::Interrupt) {
            anyhow::bail!("只有計時中可以切換類別");
        }
        let slot_id = state
            .current_slot_id
            .ok_or_else(|| anyhow::anyhow!("No active task"))?;
        let current = self
            .store
            .get_slot(slot_id)?
            .ok_or_else(|| anyhow::anyhow!("找不到這個時段"))?;
        if !current.slot_type.has_category() {
            anyhow::bail!("午休沒有類別");
        }
        if current.category_id == category_id {
            return Ok(slot_id);
        }

        let now = Local::now();
        self.store.close_slot(slot_id, now)?;
        let mut next = TimeSlot::new(current.task_name, current.slot_type, now);
        next.category_id = category_id;
        next.parent_id = current.parent_id;
        let next_id = self.store.create_slot(&next)?;

        // During an interrupt, `carried_seconds` belongs to the task underneath.
        if state.state == AppStateEnum::Working {
            state.carried_seconds = 0;
        }
        state.current_slot_id = Some(next_id);
        state.pomodoro_start = Some(now);

        Ok(next_id)
    }

    pub fn end_task(&self, note: Option<String>) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        let now = Local::now();

        if state.state == AppStateEnum::Interrupt {
            if let (Some(interrupt_id), Some(parent_id)) =
                (state.current_slot_id, state.paused_slot_id)
            {
                self.store
                    .end_interrupt_atomic(interrupt_id, parent_id, now, note)?;
            } else if let Some(interrupt_id) = state.current_slot_id {
                self.store.close_slot(interrupt_id, now)?;
            }
            Self::clear_activity(&mut state);
            return Ok(());
        }

        let slot_id = state.current_slot_id.or(state.paused_slot_id);
        if let Some(slot_id) = slot_id {
            self.store.update_slot(slot_id, Some(now), note)?;
            Self::clear_activity(&mut state);
            Ok(())
        } else {
            anyhow::bail!("No active task to end");
        }
    }

    /// Close the active or paused slot at a specific time.
    pub fn end_task_at(&self, end_ts: DateTime<Local>) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        let slot_id = state.current_slot_id.or(state.paused_slot_id);

        if let Some(slot_id) = slot_id {
            self.store.close_slot(slot_id, end_ts)?;
            Self::clear_activity(&mut state);
        } else {
            anyhow::bail!("No active task to end");
        }

        Ok(())
    }

    pub fn pause_task(&self) -> anyhow::Result<i64> {
        let mut state = self.state.lock().unwrap();

        if state.state != AppStateEnum::Working {
            anyhow::bail!("No active task to pause");
        }

        if let Some(slot_id) = state.current_slot_id {
            if let Some(start) = state.pomodoro_start {
                let elapsed = Local::now()
                    .signed_duration_since(start)
                    .num_seconds()
                    .max(0);
                let pomodoro_secs = self.store.get_settings()?.pomodoro_minutes * 60;
                state.remaining_seconds = Some((pomodoro_secs - elapsed).max(0));
            }

            state.carried_seconds = state.carried_seconds.saturating_add(segment_secs(&state));
            state.pomodoro_start = None;
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
            state.pomodoro_start = Some(Local::now());

            Ok(slot_id)
        } else {
            anyhow::bail!("No paused task to resume");
        }
    }

    pub fn start_interrupt(
        &self,
        who: Option<String>,
        reason: Option<String>,
        category_id: Option<i64>,
    ) -> anyhow::Result<i64> {
        let mut state = self.state.lock().unwrap();

        if state.state == AppStateEnum::Interrupt {
            anyhow::bail!("已經在插隊中");
        }

        let was_paused = state.state == AppStateEnum::Paused;
        let parent_id = if let Some(slot_id) = state.current_slot_id {
            let elapsed = if let Some(start) = state.pomodoro_start {
                Local::now()
                    .signed_duration_since(start)
                    .num_seconds()
                    .max(0)
            } else {
                0
            };
            if !was_paused {
                let pomodoro_secs = self.store.get_settings()?.pomodoro_minutes * 60;
                state.remaining_seconds = Some((pomodoro_secs - elapsed).max(0));
            }
            state.carried_seconds = state.carried_seconds.saturating_add(elapsed);
            state.paused_slot_id = Some(slot_id);
            Some(slot_id)
        } else {
            None
        };

        let task_name = format!("插隊：{}", who.as_deref().unwrap_or("未註明"));
        let mut slot = TimeSlot::new(task_name, SlotType::Interrupt, Local::now());
        slot.parent_id = parent_id;
        slot.note = reason;
        slot.category_id = category_id;

        let interrupt_id = self.store.create_slot(&slot)?;

        if let Some(name) = who {
            self.store.add_or_update_recent_person(&name)?;
        }

        state.state = AppStateEnum::Interrupt;
        state.return_to_paused = was_paused;
        state.current_slot_id = Some(interrupt_id);
        state.pomodoro_start = Some(Local::now());

        Ok(interrupt_id)
    }

    pub fn end_interrupt_and_resume(&self) -> anyhow::Result<Option<i64>> {
        let mut state = self.state.lock().unwrap();

        if state.state != AppStateEnum::Interrupt {
            anyhow::bail!("Not in interrupt state");
        }

        let now = Local::now();
        if let Some(interrupt_id) = state.current_slot_id {
            self.store.close_slot(interrupt_id, now)?;
        }

        if let Some(paused_id) = state.paused_slot_id {
            if state.return_to_paused {
                state.state = AppStateEnum::Paused;
                state.current_slot_id = Some(paused_id);
                state.pomodoro_start = None;
                state.return_to_paused = false;
                return Ok(Some(paused_id));
            }
            state.state = AppStateEnum::Working;
            state.current_slot_id = Some(paused_id);
            state.paused_slot_id = None;
            // `carried_seconds` is the work time before this interrupt.
            // Start a new segment so the work clock continues instead of reading zero.
            state.pomodoro_start = Some(now);

            Ok(Some(paused_id))
        } else {
            Self::clear_activity(&mut state);
            Ok(None)
        }
    }

    pub fn get_elapsed_seconds(&self) -> Option<i64> {
        let state = self.state.lock().unwrap();
        match state.state {
            AppStateEnum::Idle => None,
            AppStateEnum::Interrupt => Some(segment_secs(&state)),
            AppStateEnum::Paused => Some(state.carried_seconds),
            AppStateEnum::Working => {
                Some(state.carried_seconds.saturating_add(segment_secs(&state)))
            }
        }
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

        let id = mgr
            .start_task("Test Task".to_string(), SlotType::Work, None)
            .unwrap();
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

        let id = mgr
            .start_task("Test Task".to_string(), SlotType::Work, None)
            .unwrap();
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

        let work_id = mgr
            .start_task("Work Task".to_string(), SlotType::Work, None)
            .unwrap();
        let interrupt_id = mgr
            .start_interrupt(
                Some("Kelly".to_string()),
                Some("緊急事項".to_string()),
                None,
            )
            .unwrap();

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
        assert_eq!(interrupt_slot.note.as_deref(), Some("緊急事項"));
    }

    #[test]
    fn test_interrupt_ends_on_return_and_work_clock_continues() {
        use std::thread;
        use std::time::Duration;

        let store = Arc::new(StoreV2::new(Path::new(":memory:")).unwrap());
        let mgr = StateManager::new(store.clone());
        let work_id = mgr
            .start_task("Work".to_string(), SlotType::Work, None)
            .unwrap();
        thread::sleep(Duration::from_millis(1100));

        let interrupt_id = mgr
            .start_interrupt(Some("A".to_string()), Some("原因".to_string()), None)
            .unwrap();
        let started = mgr.get_elapsed_seconds().unwrap();
        assert!(
            started <= 1,
            "interrupt stopwatch should start over, got {started}"
        );
        thread::sleep(Duration::from_millis(1100));
        let counted = mgr.get_elapsed_seconds().unwrap();
        assert!(
            counted >= 1,
            "interrupt stopwatch should count until returning to work, got {counted}"
        );

        mgr.end_interrupt_and_resume().unwrap();
        let work_clock = mgr.get_elapsed_seconds().unwrap();
        assert!(
            work_clock >= 1,
            "work stopwatch should keep the time from before the interrupt, got {work_clock}"
        );

        let interrupt_end = store
            .get_slot(interrupt_id)
            .unwrap()
            .unwrap()
            .end_ts
            .unwrap();
        thread::sleep(Duration::from_millis(1100));
        mgr.end_task(None).unwrap();

        let work_end = store.get_slot(work_id).unwrap().unwrap().end_ts.unwrap();
        let interrupt_after = store.get_slot(interrupt_id).unwrap().unwrap();
        assert_eq!(interrupt_after.end_ts, Some(interrupt_end));
        assert!(interrupt_end < work_end);
    }

    fn setup() -> (Arc<StoreV2>, StateManager, i64, i64) {
        let store = Arc::new(StoreV2::new(Path::new(":memory:")).unwrap());
        let mgr = StateManager::new(store.clone());
        let categories = store.get_categories().unwrap();
        (store, mgr, categories[0].id, categories[1].id)
    }

    #[test]
    fn test_start_task_saves_the_category() {
        let (store, mgr, build, _) = setup();
        let id = mgr
            .start_task("Work".to_string(), SlotType::Meeting, Some(build))
            .unwrap();
        assert_eq!(
            store.get_slot(id).unwrap().unwrap().category_id,
            Some(build)
        );
    }

    #[test]
    fn test_switch_category_closes_the_slot_and_starts_a_new_one() {
        let (store, mgr, build, fix) = setup();
        let first = mgr
            .start_task("Work".to_string(), SlotType::Meeting, Some(build))
            .unwrap();

        let second = mgr.switch_category(Some(fix)).unwrap();
        assert_ne!(first, second);
        assert_eq!(mgr.get_state().current_slot_id, Some(second));
        assert_eq!(mgr.get_state().state, AppStateEnum::Working);

        let closed = store.get_slot(first).unwrap().unwrap();
        let opened = store.get_slot(second).unwrap().unwrap();
        assert_eq!(closed.category_id, Some(build));
        assert_eq!(closed.end_ts, Some(opened.start_ts));
        assert!(opened.is_ongoing());
        assert_eq!(opened.category_id, Some(fix));
        assert_eq!(opened.task_name, "Work");
        assert_eq!(opened.slot_type, SlotType::Meeting);
    }

    #[test]
    fn test_switch_to_the_same_category_keeps_the_slot() {
        let (_store, mgr, build, _) = setup();
        let id = mgr
            .start_task("Work".to_string(), SlotType::Work, Some(build))
            .unwrap();
        assert_eq!(mgr.switch_category(Some(build)).unwrap(), id);
    }

    #[test]
    fn test_switch_category_needs_a_running_timer() {
        let (_store, mgr, build, _) = setup();
        assert!(mgr.switch_category(Some(build)).is_err());

        mgr.start_task("Work".to_string(), SlotType::Work, None)
            .unwrap();
        mgr.pause_task().unwrap();
        assert!(mgr.switch_category(Some(build)).is_err());
    }

    #[test]
    fn test_lunch_cannot_switch_category() {
        let (_store, mgr, build, _) = setup();
        mgr.start_task("午休".to_string(), SlotType::Lunch, None)
            .unwrap();
        assert!(mgr.switch_category(Some(build)).is_err());
    }

    #[test]
    fn test_interrupt_has_its_own_category_and_returns_to_the_task() {
        let (store, mgr, build, fix) = setup();
        let work = mgr
            .start_task("Work".to_string(), SlotType::Work, Some(build))
            .unwrap();
        let interrupt = mgr
            .start_interrupt(Some("A".to_string()), None, Some(fix))
            .unwrap();
        assert_eq!(
            store.get_slot(interrupt).unwrap().unwrap().category_id,
            Some(fix)
        );

        assert_eq!(mgr.end_interrupt_and_resume().unwrap(), Some(work));
        assert_eq!(mgr.get_state().state, AppStateEnum::Working);
        assert_eq!(mgr.get_state().current_slot_id, Some(work));
        let current = store.get_current_slot().unwrap().unwrap();
        assert_eq!(current.id, Some(work));
        assert_eq!(current.category_id, Some(build));
    }

    #[test]
    fn test_switching_during_an_interrupt_keeps_the_way_back() {
        let (store, mgr, build, fix) = setup();
        let work = mgr
            .start_task("Work".to_string(), SlotType::Work, Some(build))
            .unwrap();
        let first = mgr
            .start_interrupt(Some("A".to_string()), None, Some(fix))
            .unwrap();

        let second = mgr.switch_category(Some(build)).unwrap();
        let switched = store.get_slot(second).unwrap().unwrap();
        assert_eq!(switched.parent_id, Some(work));
        assert_eq!(switched.slot_type, SlotType::Interrupt);
        assert!(store.get_slot(first).unwrap().unwrap().end_ts.is_some());
        assert_eq!(mgr.get_state().paused_slot_id, Some(work));

        assert_eq!(mgr.end_interrupt_and_resume().unwrap(), Some(work));
        assert!(store.get_slot(second).unwrap().unwrap().end_ts.is_some());
    }

    #[test]
    fn test_interrupting_a_paused_task_returns_to_paused() {
        let (_store, mgr, _, fix) = setup();
        let work = mgr
            .start_task("Work".to_string(), SlotType::Work, None)
            .unwrap();
        mgr.pause_task().unwrap();
        let remaining = mgr.get_state().remaining_seconds;

        mgr.start_interrupt(Some("A".to_string()), None, Some(fix))
            .unwrap();
        mgr.end_interrupt_and_resume().unwrap();

        let state = mgr.get_state();
        assert_eq!(state.state, AppStateEnum::Paused);
        assert_eq!(state.current_slot_id, Some(work));
        assert_eq!(state.paused_slot_id, Some(work));
        assert_eq!(state.remaining_seconds, remaining);
        mgr.resume_task().unwrap();
        assert_eq!(mgr.get_state().state, AppStateEnum::Working);
    }

    #[test]
    fn test_second_interrupt_cannot_lose_the_original_task() {
        let (_store, mgr, _, _) = setup();
        let work = mgr
            .start_task("Work".to_string(), SlotType::Work, None)
            .unwrap();
        mgr.start_interrupt(Some("A".to_string()), None, None)
            .unwrap();
        assert!(mgr
            .start_interrupt(Some("B".to_string()), None, None)
            .is_err());
        assert_eq!(mgr.get_state().paused_slot_id, Some(work));
    }
}
