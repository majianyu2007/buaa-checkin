use rand::Rng;
use tracing::debug;

use super::{Task, TaskQueue};

/// Parse an iclass datetime string "YYYY-MM-DD HH:MM[:SS]" → UNIX seconds (UTC+8→UTC).
pub fn parse_class_time(s: &str) -> Option<u64> {
    let fmt_full =
        time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    let fmt_short = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]");

    let pdt = time::PrimitiveDateTime::parse(s, fmt_full)
        .or_else(|_| time::PrimitiveDateTime::parse(s, fmt_short))
        .ok()?;

    let offset = time::UtcOffset::from_hms(8, 0, 0).ok()?;
    let odt = pdt.assume_offset(offset);
    let unix = odt.unix_timestamp();
    if unix < 0 {
        return None;
    }
    Some(unix as u64)
}

/// Compute a random run_at within `[class_start - 10 min, class_start]`.
pub fn randomized_run_at(class_start_secs: u64) -> u64 {
    let offset_secs: u64 = rand::rng().random_range(0..=600);
    class_start_secs.saturating_sub(offset_secs)
}

/// Enqueue tasks for a single student based on their schedules.
pub async fn plan_tasks(
    queue: &TaskQueue,
    student_id: &str,
    schedules: &[crate::client::Schedule],
    course_ids: &[String],
) {
    let now = super::now_secs();

    for sched in schedules {
        if !course_ids.contains(&sched.course_id) {
            continue;
        }
        if sched.signed() {
            continue;
        }
        let Some(class_start) = parse_class_time(&sched.time) else {
            continue;
        };
        let class_end = sched
            .end_time
            .as_deref()
            .and_then(parse_class_time)
            .unwrap_or(class_start + 5400); // default to 90 mins if missing

        let checkin_window_start = class_start.saturating_sub(600); // 10 mins before

        if now > class_end {
            // Class already ended
            continue;
        }

        let run_at = if now < checkin_window_start {
            // Future class: pick a random time in the 10-minute window [start-10m, start]
            randomized_run_at(class_start)
        } else if now < class_start {
            // We are already inside the 10-min window before class starts.
            // Pick a random time between now and class_start, with at least some jitter.
            let remaining = class_start.saturating_sub(now);
            let offset = if remaining > 30 {
                rand::rng().random_range(1..=remaining)
            } else {
                rand::rng().random_range(1..=30)
            };
            now + offset
        } else {
            // Ongoing class (we woke up late or just started): run within 30-120 seconds
            let offset: u64 = rand::rng().random_range(30..=120);
            now + offset
        };

        debug!(
            student = student_id,
            sched_id = %sched.id,
            course_id = %sched.course_id,
            run_at,
            class_start,
            class_end,
            "scheduling task"
        );
        queue
            .push(Task {
                run_at,
                student_id: student_id.to_owned(),
                schedule_id: sched.id.clone(),
                course_id: sched.course_id.clone(),
            })
            .await;
    }
}
