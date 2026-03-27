use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use tracing::{info, warn};

use crate::webhook::{self, CheckinEvent};
use crate::AppState;
use super::poller::today_str;

/// Long-running task: consume queued tasks when they become due.
pub async fn run(state: Arc<AppState>) {
    loop {
        while let Some(task) = state.queue.pop_ready().await {
            let s = state.clone();
            tokio::spawn(async move {
                execute_task(&s, &task.student_id, &task.schedule_id, &task.course_id).await;
            });
        }

        let sleep_secs = state.queue.secs_until_next().await.unwrap_or(60).min(60);
        if sleep_secs > 0 {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(sleep_secs)) => {}
                _ = state.queue.wait() => {}
            }
        }
    }
}

/// Execute one check-in task with dynamic validation and one retry on transient failure.
async fn execute_task(state: &AppState, student_id: &str, schedule_id: &str, course_id: &str) {
    let today = today_str();
    // Invalidate cache before checking to get fresh data
    state.client.invalidate_schedule_cache(student_id, &today);
    let schedules = match state.client.query_schedule(student_id, &today).await {
        Ok(s) => s,
        Err(e) => {
            warn!(student = student_id, err = %e, "pre-checkin schedule fetch failed; skipping");
            return;
        }
    };

    let target = schedules.iter().find(|s| s.course_id == course_id);
    let course_name = target.map(|s| s.name.clone()).unwrap_or_default();
    match target {
        None => {
            info!(student = student_id, sched = schedule_id, course_id, "task skipped: not found in today's schedule");
            return;
        }
        Some(s) if s.signed() => {
            info!(student = student_id, sched = schedule_id, course_id, "task skipped: already signed");
            return;
        }
        _ => {}
    }

    // Resolve student name for webhook
    let student_name = state
        .store
        .get_student(student_id)
        .map(|s| s.name)
        .unwrap_or_else(|| student_id.to_string());

    match do_checkin(state, student_id, schedule_id).await {
        Ok(_) => {
            info!(student = student_id, sched = schedule_id, course_id, "checkin ok");
            send_webhook(state, student_id, &student_name, &course_name, course_id, schedule_id, false).await;
        }
        Err(e) => {
            let jitter: u64 = rand::rng().random_range(1..=5);
            warn!(student = student_id, sched = schedule_id, course_id, err = %e, retry_secs = jitter, "checkin failed; retrying");
            tokio::time::sleep(Duration::from_secs(jitter)).await;

            match do_checkin(state, student_id, schedule_id).await {
                Ok(_) => {
                    info!(student = student_id, sched = schedule_id, course_id, "checkin ok (retry)");
                    send_webhook(state, student_id, &student_name, &course_name, course_id, schedule_id, true).await;
                }
                Err(e2) => {
                    warn!(student = student_id, sched = schedule_id, course_id, err = %e2, "checkin failed after retry");
                }
            }
        }
    }
}

async fn do_checkin(
    state: &AppState,
    student_id: &str,
    course_sched_id: &str,
) -> crate::error::AppResult<()> {
    state.client.checkin(student_id, course_sched_id).await?;
    Ok(())
}

async fn send_webhook(
    state: &AppState,
    student_id: &str,
    student_name: &str,
    course_name: &str,
    course_id: &str,
    schedule_id: &str,
    is_retry: bool,
) {
    let config = state.store.webhook();
    webhook::notify(
        &state.webhook_http,
        &config,
        &CheckinEvent {
            student_id: student_id.to_string(),
            student_name: student_name.to_string(),
            course_name: course_name.to_string(),
            course_id: course_id.to_string(),
            schedule_id: schedule_id.to_string(),
            is_retry,
        },
    )
    .await;
}
