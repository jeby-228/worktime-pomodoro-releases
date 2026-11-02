export interface TimeSlot {
  id: string | number;
  slot_type: "work" | "interrupt" | "meeting" | "lunch";
  task_title?: string;
  task_name?: string;
  person?: string | null;
  note: string | null;
  start_time?: string;
  end_time?: string | null;
  start_ts?: string | number;
  end_ts?: string | number | null;
  parent_id?: string | number | null;
  duration_minutes?: number;
  category_id?: number | null;
  category_name?: string | null;
  category_code?: string | null;
}

export interface Category {
  id: number;
  name: string;
  report_code: string | null;
  sort_order: number;
  hidden: boolean;
}

export interface CategoryReportRow {
  category_id: number | null;
  name: string | null;
  total_seconds: number;
  interrupt_seconds: number;
  percentage: number;
}

export interface CategoryReportDayItem {
  category_id: number | null;
  seconds: number;
  interrupt_seconds: number;
}

export interface CategoryReportDay {
  date: string;
  total_seconds: number;
  items: CategoryReportDayItem[];
}

export interface CategoryReport {
  start_date: string;
  end_date: string;
  total_seconds: number;
  rows: CategoryReportRow[];
  days: CategoryReportDay[];
}

export interface Gap {
  start_time: string; // ISO 8601
  end_time: string; // ISO 8601
  duration_minutes: number;
}

export interface DayStats {
  work_minutes: number;
  interrupt_minutes: number;
  meeting_minutes: number;
  lunch_minutes: number;
  total_recorded: number;
}

export interface AppState {
  status: "idle" | "working" | "paused" | "interrupt";
  current_slot_id: string | null;
  task_title: string | null;
  person: string | null;
  elapsed_seconds: number;
  remaining_seconds: number;
}

export interface RecentTask {
  task_title: string;
  last_used: string; // ISO 8601
}

export interface RecentPerson {
  person: string;
  last_used: string; // ISO 8601
}
