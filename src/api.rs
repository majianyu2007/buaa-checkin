use axum::extract::{Path, Query, State};

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

#[derive(Serialize)]
struct SystemInfo {
    version: String,
    is_admin: bool,
}

#[derive(Deserialize)]
pub struct CheckinRequest {
    schedule_id: String,
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
    let me_routes = Router::new()
        .route("/courses", get(my_courses_handler))
        .route("/courses/all", get(all_courses_handler))
        .route("/courses/{course_id}", post(add_my_course_handler))
        .route("/courses/{course_id}", delete(remove_my_course_handler));

    Router::new()
        .route("/api/login", post(login_handler))
        .route("/api/system/info", get(system_info_handler))
        .route("/api/schedules", get(schedules_handler))
        .route(
            "/api/courses/{course_id}/schedules",
            get(course_schedules_handler),
        )
        .route("/api/checkin", post(checkin_handler))
        .route("/api/users", get(list_users_handler))
        .route("/api/users/{student_id}", delete(remove_user_handler))
        .route("/api/tasks", get(tasks_handler))
        .route("/api/poll", post(poll_handler))
        .route("/api/webhook", get(get_webhook_handler))
        .route("/api/webhook", post(set_webhook_handler))
        .route("/api/webhook/test", post(test_webhook_handler))
        .nest("/api/me", me_routes)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let name = state.client.login(&req.student_id).await?;
    let token = create_token(&req.student_id, &state.jwt_secret)?;
    
    // Ensure student exists in store with their real name
    let mut entry = state.store.get_student(&req.student_id).unwrap_or_else(|| StudentEntry {
        student_id: req.student_id.clone(),
        name: name.clone(),
        course_ids: vec![],
    });
    entry.name = name.clone();
    let _ = state.store.add_student(entry);

    info!(student = %req.student_id, "user logged in via API");
    Ok(Json(LoginResponse {
        token,
        student_id: req.student_id,
        name,
    }))
}

async fn system_info_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<SystemInfo>, AppError> {
    let is_admin = extract_student_id(&headers, &state.jwt_secret)
        .map(|id| state.store.is_admin(&id))
        .unwrap_or(false);

    Ok(Json(SystemInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        is_admin,
    }))
}

// ── Me Handlers ───────────────────────────────────────────────────────────────

async fn my_courses_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<String>>, AppError> {
    let student_id = extract_student_id(&headers, &state.jwt_secret)?;
    let course_ids = state.store.get_student(&student_id).map(|s| s.course_ids).unwrap_or_default();
    Ok(Json(course_ids))
}

async fn all_courses_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<crate::client::Course>>, AppError> {
    let student_id = extract_student_id(&headers, &state.jwt_secret)?;
    let term = current_term_id();
    let courses = state.client.query_all_courses(&student_id, &term).await?;
    Ok(Json(courses))
}

fn current_term_id() -> String {
    use chrono::{Datelike, Local};
    let now = Local::now();
    let year = now.year();
    let month = now.month();

    if month >= 8 {
        format!("{}{}{}", year, year + 1, 1)
    } else {
        format!("{}{}{}", year - 1, year, 2)
    }
}

async fn add_my_course_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(course_id): Path<String>,
) -> Result<Json<MessageResponse>, AppError> {
    let student_id = extract_student_id(&headers, &state.jwt_secret)?;
    // The student must exist in the store since they logged in successfully
    let mut entry = state.store.get_student(&student_id).unwrap_or_else(|| StudentEntry {
        student_id: student_id.clone(),
        name: student_id.clone(),
        course_ids: vec![],
    });

    if !entry.course_ids.contains(&course_id) {
        entry.course_ids.push(course_id.clone());
        state.store.add_student(entry.clone())?;
        state.cache.set(student_id.clone(), entry.course_ids.clone());
    }

    Ok(Json(MessageResponse { message: "已开启自动签到".to_string() }))
}

async fn remove_my_course_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(course_id): Path<String>,
) -> Result<Json<MessageResponse>, AppError> {
    let student_id = extract_student_id(&headers, &state.jwt_secret)?;
    if let Some(mut entry) = state.store.get_student(&student_id) {
        entry.course_ids.retain(|id| id != &course_id);
        state.store.add_student(entry.clone())?;
        state.cache.set(student_id.clone(), entry.course_ids.clone());
    }

    Ok(Json(MessageResponse { message: "已关闭自动签到".to_string() }))
}

// ── Action Handlers ───────────────────────────────────────────────────────────

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
    let schedules = state.client.query_course_schedule(&student_id, &course_id).await?;
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

// ── Admin Handlers ────────────────────────────────────────────────────────────

fn ensure_admin(headers: &axum::http::HeaderMap, state: &AppState) -> AppResult<String> {
    let student_id = extract_student_id(headers, &state.jwt_secret)?;
    if !state.store.is_admin(&student_id) {
        return Err(AppError::unauthorized("需要管理员权限"));
    }
    Ok(student_id)
}

async fn list_users_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let _admin = ensure_admin(&headers, &state)?;
    let users = state.store.students().into_iter().map(|s| UserResponse {
        student_id: s.student_id,
        name: s.name,
        course_ids: s.course_ids,
    }).collect();
    Ok(Json(users))
}

async fn remove_user_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(student_id): Path<String>,
) -> Result<Json<MessageResponse>, AppError> {
    let _admin = ensure_admin(&headers, &state)?;
    let removed = state.store.remove_student(&student_id)?;
    if removed {
        state.cache.remove(&student_id);
        info!(student = %student_id, "user removed by admin");
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
    let _admin = ensure_admin(&headers, &state)?;
    let tasks = state.queue.snapshot().await.into_iter().map(|t| TaskResponse {
        run_at: t.run_at,
        student_id: t.student_id,
        schedule_id: t.schedule_id,
        course_id: t.course_id,
    }).collect();
    Ok(Json(tasks))
}

async fn poll_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<MessageResponse>, AppError> {
    let _admin = ensure_admin(&headers, &state)?;
    poller::poll_once(&state).await;
    Ok(Json(MessageResponse { message: "已手动触发轮询".to_string() }))
}

async fn get_webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<WebhookConfig>, AppError> {
    let _admin = ensure_admin(&headers, &state)?;
    Ok(Json(state.store.webhook()))
}

async fn set_webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(config): Json<WebhookConfig>,
) -> Result<Json<MessageResponse>, AppError> {
    let _admin = ensure_admin(&headers, &state)?;
    state.store.set_webhook(config)?;
    info!("webhook config updated via API");
    Ok(Json(MessageResponse { message: "Webhook 配置已更新".to_string() }))
}

async fn test_webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<MessageResponse>, AppError> {
    let _admin = ensure_admin(&headers, &state)?;
    let config = state.store.webhook();
    if !config.enabled {
        return Err(AppError::bad_request("Webhook 通知未启用"));
    }
    let event = crate::webhook::CheckinEvent {
        student_id: "ADMIN".to_string(),
        student_name: "系统管理员".to_string(),
        course_name: "测试通知课程".to_string(),
        course_id: "TEST-1234".to_string(),
        schedule_id: "SCHED-TEST-0000".to_string(),
        is_retry: false,
    };
    crate::webhook::notify(&state.webhook_http, &config, &event).await;
    Ok(Json(MessageResponse { message: "测试通知已触发".to_string() }))
}
