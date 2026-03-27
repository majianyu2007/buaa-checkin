mod api;
mod client;
mod error;
mod pipeline;
mod store;
mod webhook;

use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::info;

pub struct AppState {
    pub client: Arc<client::ClassClient>,
    pub cache: Arc<pipeline::SchedulerCache>,
    pub queue: Arc<pipeline::TaskQueue>,
    pub store: Arc<store::Store>,
    pub webhook_http: reqwest::Client,
    pub jwt_secret: String,
    pub poll_interval_minutes: u64,
}

#[tokio::main]
async fn main() {
    // Init logging
    let env = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "buaa_checkin=info".into());
    tracing_subscriber::fmt().with_env_filter(env).init();

    // Config
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let config_path = format!("{}/config.json", data_dir);
    let jwt_secret =
        std::env::var("JWT_SECRET").unwrap_or_else(|_| "buaa-checkin-default-secret".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);

    // Load store
    let store = Arc::new(store::Store::load(&config_path));
    let config = store.config();

    info!(
        config = %config_path,
        students = config.students.len(),
        poll_interval = config.poll_interval_minutes,
        "starting buaa-checkin"
    );

    let client = Arc::new(client::ClassClient::new());
    let cache = Arc::new(pipeline::SchedulerCache::new());
    let queue = Arc::new(pipeline::TaskQueue::new());
    let webhook_http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build webhook http client");

    // Load students into scheduler cache
    for student in &config.students {
        cache.set(student.student_id.clone(), student.course_ids.clone());
        info!(student = %student.student_id, name = %student.name, "loaded student config");
    }

    let state = Arc::new(AppState {
        client,
        cache,
        queue,
        store,
        webhook_http,
        jwt_secret,
        poll_interval_minutes: config.poll_interval_minutes,
    });

    // Launch background pipeline tasks
    let poller_state = state.clone();
    tokio::spawn(async move {
        pipeline::poller::run(poller_state).await;
    });

    let worker_state = state.clone();
    tokio::spawn(async move {
        pipeline::worker::run(worker_state).await;
    });

    // CORS layer (permissive for development / single-deployment use)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build Axum app
    let app = api::routes()
        .layer(cors)
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    info!(addr = %addr, "HTTP server starting");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
