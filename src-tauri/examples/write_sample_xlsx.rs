use chrono::NaiveDate;
use worktime_pomodoro_lib::{write_xlsx, ExportRow};

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: write_sample_xlsx <out.xlsx>");

    let rows = vec![
        ExportRow {
            date: NaiveDate::from_ymd_opt(2026, 10, 21).unwrap(),
            start_time: "09:00".into(),
            end_time: "10:25".into(),
            minutes: 85,
            task_name: "計劃A｜階段1｜開發功能".into(),
        },
        ExportRow {
            date: NaiveDate::from_ymd_opt(2026, 10, 21).unwrap(),
            start_time: "10:30".into(),
            end_time: "11:15".into(),
            minutes: 45,
            task_name: "週例會".into(),
        },
        ExportRow {
            date: NaiveDate::from_ymd_opt(2026, 10, 21).unwrap(),
            start_time: "23:00".into(),
            end_time: "00:00".into(),
            minutes: 60,
            task_name: "值班｜深夜部署".into(),
        },
        ExportRow {
            date: NaiveDate::from_ymd_opt(2026, 10, 22).unwrap(),
            start_time: "00:00".into(),
            end_time: "01:00".into(),
            minutes: 60,
            task_name: "值班｜深夜部署".into(),
        },
        ExportRow {
            date: NaiveDate::from_ymd_opt(2026, 10, 22).unwrap(),
            start_time: "09:00".into(),
            end_time: "09:45".into(),
            minutes: 45,
            task_name: "代碼審查".into(),
        },
    ];

    let total = write_xlsx(&path, &rows).expect("write xlsx");
    println!("wrote {} ({} minutes)", path, total);
}
