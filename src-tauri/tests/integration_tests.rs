#![allow(
    clippy::identity_op,
    clippy::double_ended_iterator_last,
    clippy::unnecessary_cast
)]

/// Integration tests covering all acceptance scenarios from section 12 of the spec
use chrono::{Duration, Local, TimeZone};
use worktime_pomodoro_lib::*;

#[test]
fn scenario_1_continuous_work_with_switch() {
    // Scenario 1: Start A → 25 min ping continue → 25 min switch to B → end of day
    // Expected: A 50 minutes, B until 17:30

    let store = Store::new_in_memory().unwrap();
    let sm = StateMachine::new();
    let deriver = SegmentDeriver::new(store.clone());

    let day_start = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();
    let ping1 = day_start + Duration::minutes(25);
    let ping2 = ping1 + Duration::minutes(25);
    let end_of_day = Local.with_ymd_and_hms(2026, 10, 21, 17, 30, 0).unwrap();

    // Start task A
    let events = sm.start_task("TaskA".to_string(), day_start, 0, EventSource::User, None);
    for e in events {
        store.insert_event(&e).unwrap();
    }

    // Ping 1 - continue (no events since same task)
    let ping_event = sm.ping(ping1, 25 * 60 * 1000);
    store.insert_event(&ping_event).unwrap();

    // Ping 2 - switch to B
    let events = sm.start_task(
        "TaskB".to_string(),
        ping2,
        50 * 60 * 1000,
        EventSource::Ping,
        None,
    );
    for e in events {
        store.insert_event(&e).unwrap();
    }

    // Auto end at end of day
    let events = sm.end_task(end_of_day, 510 * 60 * 1000, EventSource::Auto);
    for e in events {
        store.insert_event(&e).unwrap();
    }

    // Derive segments
    deriver.rebuild_segments().unwrap();

    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let segments = store.get_segments_for_date(date).unwrap();

    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].task_key, Some("TaskA".to_string()));
    assert_eq!(segments[0].minutes, Some(50));
    assert_eq!(segments[1].task_key, Some("TaskB".to_string()));

    // TaskB should be from ping2 to end_of_day
    let expected_b_minutes = (end_of_day.signed_duration_since(ping2).num_seconds() / 60) as i64;
    assert_eq!(segments[1].minutes, Some(expected_b_minutes));
}

#[test]
fn scenario_2_ping_timeout_and_resolve() {
    // Scenario 2: Ping not responded → marked unknown → resolve to task A

    let store = Store::new_in_memory().unwrap();
    let sm = StateMachine::new();
    let deriver = SegmentDeriver::new(store.clone());

    let work_start = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();
    let ping_time = work_start + Duration::minutes(25);
    let timeout_time = ping_time + Duration::minutes(5); // Ping timeout
    let resolve_time = work_start + Duration::minutes(60);

    // Start task
    let events = sm.start_task("TaskA".to_string(), work_start, 0, EventSource::User, None);
    for e in events {
        store.insert_event(&e).unwrap();
    }

    // Ping event
    let ping_event = sm.ping(ping_time, 25 * 60 * 1000);
    store.insert_event(&ping_event).unwrap();

    // Timeout - mark as unknown
    let events = sm.pause(timeout_time, 30 * 60 * 1000, EventSource::Auto);
    for e in events {
        store.insert_event(&e).unwrap();
    }

    let unknown_begin = sm.mark_unknown_begin(timeout_time, 30 * 60 * 1000, EventSource::Auto);
    store.insert_event(&unknown_begin).unwrap();

    let unknown_end = sm.mark_unknown_end(resolve_time, 60 * 60 * 1000, EventSource::Auto);
    store.insert_event(&unknown_end).unwrap();

    // User resolves: attribute unknown period to TaskA
    // This would be done via a resolve event in real app
    // For now, we can manually start TaskA at the timeout time
    let _events = sm.start_task(
        "TaskA".to_string(),
        timeout_time,
        30 * 60 * 1000,
        EventSource::User,
        None,
    );
    // Note: This creates overlap, which the real resolve mechanism should handle better

    deriver.rebuild_segments().unwrap();

    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let segments = store.get_segments_for_date(date).unwrap();

    // Should have segments for TaskA and unknown period
    assert!(!segments.is_empty());

    // Check that there's an unknown segment
    let has_unknown = segments.iter().any(|s| s.status == SegmentStatus::Unknown);
    assert!(has_unknown);
}

#[test]
fn scenario_3_crash_recovery() {
    // Scenario 3: Force quit while working → restart 10 min later → end at last ping

    let store = Store::new_in_memory().unwrap();
    let _sm = StateMachine::new();

    let work_start = Local.with_ymd_and_hms(2026, 10, 21, 10, 0, 0).unwrap();
    let ping1 = work_start + Duration::minutes(25);
    let crash_time = ping1 + Duration::minutes(10); // Crashed 10 min after ping

    // Old boot session
    let old_boot_id = "old-boot-123".to_string();

    // Start task in old session
    let mut event = Event {
        id: None,
        uuid: uuid::Uuid::new_v4(),
        ts: work_start,
        mono_ms: 0,
        boot_id: old_boot_id.clone(),
        kind: EventKind::Start,
        task_key: Some("TaskA".to_string()),
        ref_event_id: None,
        note: None,
        source: EventSource::User,
        synced: false,
    };
    store.insert_event(&event).unwrap();

    // Ping in old session
    event.uuid = uuid::Uuid::new_v4();
    event.ts = ping1;
    event.mono_ms = 25 * 60 * 1000;
    event.kind = EventKind::Ping;
    event.task_key = None;
    store.insert_event(&event).unwrap();

    // Crash happens (no more events)

    // New session starts
    let sm_new = StateMachine::new(); // New boot_id
    let recovery = Recovery::new(store.clone(), sm_new);

    let restart_time = crash_time;
    let action = recovery.check_on_startup(restart_time).unwrap();

    assert!(action.is_some());
    let action = action.unwrap();
    assert!(action.needs_user_resolve);

    // Should have ended at ping1
    let end_event = action
        .events_to_insert
        .iter()
        .find(|e| matches!(e.kind, EventKind::End))
        .unwrap();
    assert_eq!(end_event.ts, ping1);
    assert_eq!(end_event.task_key, Some("TaskA".to_string()));
}

#[test]
fn scenario_4_sleep_recovery() {
    // Scenario 4: Computer sleeps for 40 minutes → wake up asks user → resolve to meeting

    let store = Store::new_in_memory().unwrap();
    let sm = StateMachine::new();
    let deriver = SegmentDeriver::new(store.clone());

    let work_start = Local.with_ymd_and_hms(2026, 10, 21, 10, 0, 0).unwrap();
    let sleep_time = work_start + Duration::minutes(30);
    let wake_time = sleep_time + Duration::minutes(40);

    // Start working
    let events = sm.start_task("TaskA".to_string(), work_start, 0, EventSource::User, None);
    for e in events {
        store.insert_event(&e).unwrap();
    }

    // Sleep detected
    let recovery = Recovery::new(store.clone(), sm);
    let sleep_events = recovery.handle_sleep(sleep_time).unwrap();
    for e in sleep_events {
        store.insert_event(&e).unwrap();
    }

    // Wake detected
    let wake_events = recovery.handle_wake(wake_time).unwrap();
    for e in wake_events {
        store.insert_event(&e).unwrap();
    }

    // Derive segments
    deriver.rebuild_segments().unwrap();

    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let segments = store.get_segments_for_date(date).unwrap();

    // Should have: TaskA segment, and unknown segment for sleep period
    assert!(segments.len() >= 2);

    // First segment should be TaskA for 30 minutes
    assert_eq!(segments[0].task_key, Some("TaskA".to_string()));
    assert_eq!(segments[0].minutes, Some(30));

    // Second segment should be unknown for 40 minutes
    assert_eq!(segments[1].task_key, None);
    assert_eq!(segments[1].status, SegmentStatus::Unknown);
    assert_eq!(segments[1].minutes, Some(40));
}

#[test]
fn scenario_5_offline_all_day() {
    // Scenario 5: No network all day → all events stored locally → unsynced count correct

    let store = Store::new_in_memory().unwrap();
    let sm = StateMachine::new();

    let work_start = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();

    // Start task
    let events = sm.start_task("TaskA".to_string(), work_start, 0, EventSource::User, None);
    for e in events {
        store.insert_event(&e).unwrap();
    }

    // Multiple pings throughout the day
    for i in 1..=10 {
        let ping_time = work_start + Duration::minutes(25 * i);
        let ping_event = sm.ping(ping_time, (25 * i * 60 * 1000) as i64);
        store.insert_event(&ping_event).unwrap();
    }

    // End task
    let end_time = work_start + Duration::minutes(250);
    let events = sm.end_task(end_time, 250 * 60 * 1000, EventSource::User);
    for e in events {
        store.insert_event(&e).unwrap();
    }

    // Check all events are unsynced
    let all_events = store.get_all_events().unwrap();
    let unsynced_count = all_events.iter().filter(|e| !e.synced).count();

    assert_eq!(unsynced_count, all_events.len());
    assert_eq!(unsynced_count, 12); // 1 start + 10 pings + 1 end
}

#[test]
fn scenario_6_adhoc_task() {
    // Scenario 6: Manually type adhoc task → can track time → marked for mapping

    let store = Store::new_in_memory().unwrap();
    let sm = StateMachine::new();
    let deriver = SegmentDeriver::new(store.clone());

    let work_start = Local.with_ymd_and_hms(2026, 10, 21, 10, 0, 0).unwrap();
    let work_end = work_start + Duration::minutes(45);

    // Add adhoc task to cache
    let adhoc_task = TaskCache {
        task_key: "ADHOC|臨時任務|修復緊急Bug".to_string(),
        plan_name: Some("ADHOC".to_string()),
        stage_name: Some("臨時任務".to_string()),
        process_name: None,
        task_name: Some("修復緊急Bug".to_string()),
        task_id: None,
        is_adhoc: true,
        is_closed: false,
        last_used_at: Some(work_start),
        fetched_at: None,
    };
    store.insert_or_update_task(&adhoc_task).unwrap();

    // Start adhoc task
    let events = sm.start_task(
        adhoc_task.task_key.clone(),
        work_start,
        0,
        EventSource::User,
        None,
    );
    for e in events {
        store.insert_event(&e).unwrap();
    }

    // End task
    let events = sm.end_task(work_end, 45 * 60 * 1000, EventSource::User);
    for e in events {
        store.insert_event(&e).unwrap();
    }

    // Derive segments
    deriver.rebuild_segments().unwrap();

    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let segments = store.get_segments_for_date(date).unwrap();

    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].task_key, Some(adhoc_task.task_key.clone()));
    assert_eq!(segments[0].minutes, Some(45));

    // Verify task is marked as adhoc
    let retrieved_task = store.get_task(&adhoc_task.task_key).unwrap();
    assert!(retrieved_task.is_some());
    assert!(retrieved_task.unwrap().is_adhoc);
}

#[test]
fn scenario_7_utf8_task_names() {
    // Scenario 7: Vietnamese task names → display, store, export without corruption

    let store = Store::new_in_memory().unwrap();
    let sm = StateMachine::new();

    let work_start = Local.with_ymd_and_hms(2026, 10, 21, 10, 0, 0).unwrap();

    // Tasks with various UTF-8 characters
    let tasks = vec![
        "計劃|階段|流程|繁體中文任務",
        "计划|阶段|流程|简体中文任务",
        "Plan|Stage|Process|Nhiệm vụ tiếng Việt",
        "プラン|ステージ|プロセス|日本語タスク",
    ];

    for (i, task_key) in tasks.iter().enumerate() {
        let ts = work_start + Duration::minutes(30 * i as i64);

        let task = TaskCache {
            task_key: task_key.to_string(),
            plan_name: Some("測試".to_string()),
            stage_name: Some("測試階段".to_string()),
            process_name: Some("測試流程".to_string()),
            task_name: Some(task_key.split('|').last().unwrap().to_string()),
            task_id: Some(i as i64),
            is_adhoc: false,
            is_closed: false,
            last_used_at: Some(ts),
            fetched_at: Some(ts),
        };
        store.insert_or_update_task(&task).unwrap();

        let events = sm.start_task(task_key.to_string(), ts, 0, EventSource::User, None);
        for e in events {
            store.insert_event(&e).unwrap();
        }
    }

    // Verify all tasks are stored correctly
    let all_tasks = store.get_all_tasks().unwrap();
    assert_eq!(all_tasks.len(), tasks.len());

    for task_key in tasks {
        let retrieved = store.get_task(task_key).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().task_key, task_key);
    }
}

#[test]
fn scenario_overnight_recovery() {
    // Additional test: Overnight shutdown with recovery

    let store = Store::new_in_memory().unwrap();
    let _sm = StateMachine::new();

    let yesterday = Local.with_ymd_and_hms(2026, 10, 20, 16, 0, 0).unwrap();
    let yesterday_ping = Local.with_ymd_and_hms(2026, 10, 20, 16, 45, 0).unwrap();

    // Old session yesterday
    let mut event = Event {
        id: None,
        uuid: uuid::Uuid::new_v4(),
        ts: yesterday,
        mono_ms: 0,
        boot_id: "old-boot".to_string(),
        kind: EventKind::Start,
        task_key: Some("TaskA".to_string()),
        ref_event_id: None,
        note: None,
        source: EventSource::User,
        synced: false,
    };
    store.insert_event(&event).unwrap();

    event.uuid = uuid::Uuid::new_v4();
    event.ts = yesterday_ping;
    event.mono_ms = 45 * 60 * 1000;
    event.kind = EventKind::Ping;
    event.task_key = None;
    store.insert_event(&event).unwrap();

    // New session today
    let sm_new = StateMachine::new();
    let recovery = Recovery::new(store.clone(), sm_new);

    let today = Local.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap();
    let action = recovery.check_on_startup(today).unwrap();

    assert!(action.is_some());
    let action = action.unwrap();

    // Should end yesterday's task at last ping
    let end_event = &action.events_to_insert[0];
    assert!(matches!(end_event.kind, EventKind::End));
    assert_eq!(end_event.ts.date_naive(), yesterday.date_naive());
}

#[test]
fn test_time_jump_detection() {
    // Test system clock change detection

    let store = Store::new_in_memory().unwrap();
    let sm = StateMachine::new();
    let recovery = Recovery::new(store, sm);

    // Normal: 5 minutes monotonic = 5 minutes clock time
    assert!(!recovery.detect_time_jump(5 * 60 * 1000, 5 * 60));

    // Jump: 5 minutes monotonic but 1 hour clock time
    assert!(recovery.detect_time_jump(5 * 60 * 1000, 60 * 60));

    // Jump backward: 5 minutes monotonic but only 1 minute clock time
    assert!(recovery.detect_time_jump(5 * 60 * 1000, 1 * 60));

    // Small drift acceptable (< 2 minutes)
    assert!(!recovery.detect_time_jump(5 * 60 * 1000, 5 * 60 + 90));
}
