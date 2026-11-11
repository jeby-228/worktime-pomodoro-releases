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

    /// Lunch is the one status that never carries a category.
    pub fn has_category(&self) -> bool {
        !matches!(self, Self::Lunch)
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
    #[serde(default)]
    pub category_id: Option<i64>,
    /// Joined from `categories` on read. `category_id` is what gets stored.
    #[serde(default)]
    pub category_name: Option<String>,
    #[serde(default)]
    pub category_code: Option<String>,
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
            category_id: None,
            category_name: None,
            category_code: None,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub report_code: Option<String>,
    pub sort_order: i64,
    pub hidden: bool,
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

/// One category's share of a report. `category_id: None` is uncategorized.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryReportRow {
    pub category_id: Option<i64>,
    pub name: Option<String>,
    pub total_seconds: i64,
    /// The part of `total_seconds` spent on interrupts.
    pub interrupt_seconds: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryReportDayItem {
    pub category_id: Option<i64>,
    pub seconds: i64,
    pub interrupt_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryReportDay {
    pub date: String,
    pub total_seconds: i64,
    pub items: Vec<CategoryReportDayItem>,
}

/// Lunch is excluded. Work and meeting time is net of the interrupts inside it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CategoryReport {
    pub start_date: String,
    pub end_date: String,
    pub total_seconds: i64,
    pub rows: Vec<CategoryReportRow>,
    pub days: Vec<CategoryReportDay>,
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
    pub theme: String,
    /// Missing values stay enabled, matching TokenTracker's auto-update default.
    #[serde(default = "default_enabled")]
    pub auto_update: bool,
    /// Pre-select the last used category when starting a timer.
    #[serde(default = "default_enabled")]
    pub remember_last_category: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            lunch_start: "12:00".to_string(),
            lunch_end: "13:00".to_string(),
            work_end: "17:20".to_string(),
            eod_check_enabled: true,
            theme: "system".to_string(),
            auto_update: true,
            remember_last_category: true,
        }
    }
}
