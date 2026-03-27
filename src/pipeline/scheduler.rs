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
        let run_at = randomized_run_at(class_start);
        if run_at < now {
            continue;
        }
        debug!(
            student = student_id,
            sched_id = %sched.id,
            course_id = %sched.course_id,
            run_at,
            class_start,
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
