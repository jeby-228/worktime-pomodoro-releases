use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SlotType {
    Work,
    Interrupt,
    Meeting,
    Lunch,
}

impl SlotType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Interrupt => "interrupt",
            Self::Meeting => "meeting",
            Self::Lunch => "lunch",
        }
    }
}

impl std::str::FromStr for SlotType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "work" => Ok(Self::Work),
            "interrupt" => Ok(Self::Interrupt),
            "meeting" => Ok(Self::Meeting),
            "lunch" => Ok(Self::Lunch),
            _ => anyhow::bail!("Unknown slot type: {}", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSlot {
    pub id: Option<i64>,
    pub uuid: Uuid,
    pub task_name: String,
    pub slot_type: SlotType,
    pub start_ts: DateTime<Local>,
    pub end_ts: Option<DateTime<Local>>,
    pub note: Option<String>,
    pub parent_id: Option<i64>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

impl TimeSlot {
    pub fn new(task_name: String, slot_type: SlotType, start_ts: DateTime<Local>) -> Self {
        let now = Local::now();
        Self {
            id: None,
            uuid: Uuid::new_v4(),
            task_name,
            slot_type,
            start_ts,
            end_ts: None,
            note: None,
            parent_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn duration_minutes(&self) -> Option<i64> {
        self.end_ts.map(|end| {
            let duration = end.signed_duration_since(self.start_ts);
            duration.num_minutes()
        })
    }

    pub fn is_ongoing(&self) -> bool {
        self.end_ts.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentPerson {
    pub id: Option<i64>,
    pub name: String,
    pub last_used: DateTime<Local>,
    pub use_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentTask {
    pub id: Option<i64>,
    pub task_name: String,
    pub last_used: DateTime<Local>,
    pub use_count: i64,
    pub slot_type: SlotType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayStats {
    pub date: String,
    pub total_minutes: i64,
    pub work_minutes: i64,
    pub interrupt_minutes: i64,
    pub meeting_minutes: i64,
    pub lunch_minutes: i64,
    pub slots: Vec<TimeSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub task_name: String,
    pub slot_type: SlotType,
    pub total_minutes: i64,
    pub total_hours: f64,
    pub percentage: f64,
    pub slot_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gap {
    pub start_ts: DateTime<Local>,
    pub end_ts: DateTime<Local>,
    pub minutes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub lunch_start: String,
    pub lunch_end: String,
    pub work_end: String,
    pub eod_check_enabled: bool,
    pub pomodoro_minutes: i64,
    pub theme: String,
    pub mini_mode: String,
    pub pet_id: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            lunch_start: "12:00".to_string(),
            lunch_end: "13:00".to_string(),
            work_end: "17:20".to_string(),
            eod_check_enabled: true,
            pomodoro_minutes: 25,
            theme: "system".to_string(),
            mini_mode: "none".to_string(),
            pet_id: "tomato-kun".to_string(),
        }
    }
}
