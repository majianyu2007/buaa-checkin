use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::error::{AppError, AppResult};
use crate::pipeline::poller;
use crate::store::StudentEntry;
use crate::webhook::WebhookConfig;
use crate::AppState;

// ── JWT helpers ───────────────────────────────────────────────────────────────

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String, // student_id
    exp: usize,
}

fn create_token(student_id: &str, secret: &str) -> AppResult<String> {
    let exp = (chrono_now_secs() + 7 * 24 * 3600) as usize; // 7 days
    let claims = Claims {
        sub: student_id.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::internal(format!("jwt encode: {e}")))
}

fn verify_token(token: &str, secret: &str) -> AppResult<String> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| AppError::unauthorized("无效或已过期的登录凭据"))?;
    Ok(data.claims.sub)
}

fn chrono_now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Extract student_id from Authorization header.
fn extract_student_id(
    headers: &axum::http::HeaderMap,
    secret: &str,
) -> AppResult<String> {
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::unauthorized("缺少 Authorization 头"))?;
    let token = auth.strip_prefix("Bearer ").unwrap_or(auth);
    verify_token(token, secret)
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    student_id: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    student_id: String,
    name: String,
}

#[derive(Deserialize)]
pub struct ScheduleQuery {
    date: Option<String>,
}

#[derive(Deserialize)]
pub struct CheckinRequest {
    schedule_id: String,
}

#[derive(Deserialize)]
pub struct AddUserRequest {
    student_id: String,
    course_ids: Vec<String>,
}

#[derive(Serialize)]
struct UserResponse {
    student_id: String,
    name: String,
    course_ids: Vec<String>,
}

#[derive(Serialize)]
struct TaskResponse {
    run_at: u64,
    student_id: String,
    schedule_id: String,
    course_id: String,
}

#[derive(Serialize)]
struct MessageResponse {
    message: String,
}

// ── Route builders ────────────────────────────────────────────────────────────

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/login", post(login_handler))
        .route("/api/schedules", get(schedules_handler))
        .route(
            "/api/courses/{course_id}/schedules",
            get(course_schedules_handler),
        )
        .route("/api/checkin", post(checkin_handler))
        .route("/api/users", get(list_users_handler))
        .route("/api/users", post(add_user_handler))
        .route("/api/users/{student_id}", delete(remove_user_handler))
        .route("/api/tasks", get(tasks_handler))
        .route("/api/poll", post(poll_handler))
        .route("/api/webhook", get(get_webhook_handler))
        .route("/api/webhook", post(set_webhook_handler))
        .route("/api/webhook/test", post(test_webhook_handler))
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let name = state.client.login(&req.student_id).await?;
    let token = create_token(&req.student_id, &state.jwt_secret)?;
    info!(student = %req.student_id, "user logged in via API");
    Ok(Json(LoginResponse {
        token,
        student_id: req.student_id,
        name,
    }))
}

async fn schedules_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<ScheduleQuery>,
) -> Result<Json<Vec<crate::client::Schedule>>, AppError> {
    let student_id = extract_student_id(&headers, &state.jwt_secret)?;
    let date = q.date.unwrap_or_else(poller::today_str);
    let schedules = state.client.query_schedule(&student_id, &date).await?;
    Ok(Json(schedules))
}

async fn course_schedules_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(course_id): Path<String>,
) -> Result<Json<Vec<crate::client::CourseSchedule>>, AppError> {
    let student_id = extract_student_id(&headers, &state.jwt_secret)?;
    let schedules = state
        .client
        .query_course_schedule(&student_id, &course_id)
        .await?;
    Ok(Json(schedules))
}

async fn checkin_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CheckinRequest>,
) -> Result<Json<MessageResponse>, AppError> {
    let student_id = extract_student_id(&headers, &state.jwt_secret)?;
    state.client.checkin(&student_id, &req.schedule_id).await?;
    // Invalidate schedule cache so dashboard refreshes show new status
    let today = poller::today_str();
    state.client.invalidate_schedule_cache(&student_id, &today);
    info!(student = %student_id, sched = %req.schedule_id, "manual checkin via API");
    Ok(Json(MessageResponse {
        message: "签到成功".to_string(),
    }))
}

async fn list_users_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let _student_id = extract_student_id(&headers, &state.jwt_secret)?;
    let users: Vec<UserResponse> = state
        .store
        .students()
        .into_iter()
        .map(|s| UserResponse {
            student_id: s.student_id,
            name: s.name,
            course_ids: s.course_ids,
        })
        .collect();
    Ok(Json(users))
}

async fn add_user_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<AddUserRequest>,
) -> Result<(StatusCode, Json<MessageResponse>), AppError> {
    let _caller = extract_student_id(&headers, &state.jwt_secret)?;

    // Validate that the student_id actually exists by attempting login
    let name = state.client.login(&req.student_id).await?;

    let entry = StudentEntry {
        student_id: req.student_id.clone(),
        name: name.clone(),
        course_ids: req.course_ids.clone(),
    };
    state.store.add_student(entry)?;

    // Update the scheduler cache so the poller picks up this student
    state
        .cache
        .set(req.student_id.clone(), req.course_ids.clone());

    info!(
        student = %req.student_id,
        name = %name,
        courses = req.course_ids.len(),
        "user added to auto-checkin"
    );

    Ok((
        StatusCode::CREATED,
        Json(MessageResponse {
            message: format!("已添加用户 {} ({})", name, req.student_id),
        }),
    ))
}

async fn remove_user_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(student_id): Path<String>,
) -> Result<Json<MessageResponse>, AppError> {
    let _caller = extract_student_id(&headers, &state.jwt_secret)?;
    let removed = state.store.remove_student(&student_id)?;
    if removed {
        state.cache.remove(&student_id);
        info!(student = %student_id, "user removed from auto-checkin");
        Ok(Json(MessageResponse {
            message: format!("已删除用户 {}", student_id),
        }))
    } else {
        Err(AppError::not_found("用户不存在"))
    }
}

async fn tasks_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<TaskResponse>>, AppError> {
    let _student_id = extract_student_id(&headers, &state.jwt_secret)?;
    let tasks: Vec<TaskResponse> = state
        .queue
        .snapshot()
        .await
        .into_iter()
        .map(|t| TaskResponse {
            run_at: t.run_at,
            student_id: t.student_id,
            schedule_id: t.schedule_id,
            course_id: t.course_id,
        })
        .collect();
    Ok(Json(tasks))
}

async fn poll_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<MessageResponse>, AppError> {
    let _student_id = extract_student_id(&headers, &state.jwt_secret)?;
    poller::poll_once(&state).await;
    Ok(Json(MessageResponse {
        message: "已手动触发轮询".to_string(),
    }))
}

async fn get_webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<WebhookConfig>, AppError> {
    let _student_id = extract_student_id(&headers, &state.jwt_secret)?;
    Ok(Json(state.store.webhook()))
}

async fn set_webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(config): Json<WebhookConfig>,
) -> Result<Json<MessageResponse>, AppError> {
    let _student_id = extract_student_id(&headers, &state.jwt_secret)?;
    state.store.set_webhook(config)?;
    info!("webhook config updated via API");
    Ok(Json(MessageResponse {
        message: "Webhook 配置已更新".to_string(),
    }))
}

async fn test_webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<MessageResponse>, AppError> {
    let student_id = extract_student_id(&headers, &state.jwt_secret)?;
    let student_name = state
        .store
        .get_student(&student_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| student_id.clone());

    let config = state.store.webhook();
    if !config.enabled {
        return Err(AppError::bad_request("Webhook 通知未启用，请先配置并启用"));
    }

    let event = crate::webhook::CheckinEvent {
        student_id,
        student_name,
        course_name: "测试通知课程".to_string(),
        course_id: "TEST-1234".to_string(),
        schedule_id: "SCHED-TEST-0000".to_string(),
        is_retry: false,
    };

    crate::webhook::notify(&state.webhook_http, &config, &event).await;

    Ok(Json(MessageResponse {
        message: "测试通知已触发".to_string(),
    }))
}
