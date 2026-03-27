use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Webhook configuration for checkin notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Enable/disable webhook notifications
    #[serde(default)]
    pub enabled: bool,
    /// Webhook provider: "serverchan", "custom"
    #[serde(default = "default_provider")]
    pub provider: String,
    /// For Server酱: your SendKey.  For custom: the full URL.
    #[serde(default)]
    pub key: String,
    /// Optional: custom webhook URL (overrides provider default)
    #[serde(default)]
    pub url: Option<String>,
}

fn default_provider() -> String {
    "serverchan".to_string()
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_provider(),
            key: String::new(),
            url: None,
        }
    }
}

/// Checkin event payload for webhook notification.
pub struct CheckinEvent {
    pub student_id: String,
    pub student_name: String,
    pub course_name: String,
    pub course_id: String,
    pub schedule_id: String,
    pub is_retry: bool,
}

/// Send a webhook notification for a successful checkin.
pub async fn notify(client: &Client, config: &WebhookConfig, event: &CheckinEvent) {
    if !config.enabled || config.key.is_empty() {
        return;
    }

    let title = format!("✅ 签到成功 | {}", event.course_name);

    let body = format!(
        "**学生**: {} ({})\n\n**课程**: {} (`{}`)\n\n**Schedule ID**: `{}`\n\n**重试**: {}",
        event.student_name,
        event.student_id,
        event.course_name,
        event.course_id,
        event.schedule_id,
        if event.is_retry { "是" } else { "否" },
    );

    let result = match config.provider.as_str() {
        "serverchan" => send_serverchan(client, &config.key, &title, &body).await,
        "custom" => {
            let url = config.url.as_deref().unwrap_or(&config.key);
            send_custom(client, url, &title, &body).await
        }
        other => {
            warn!(provider = other, "unknown webhook provider, skipping");
            return;
        }
    };

    match result {
        Ok(_) => info!(
            student = %event.student_id,
            course = %event.course_id,
            provider = %config.provider,
            "webhook notification sent"
        ),
        Err(e) => warn!(
            student = %event.student_id,
            course = %event.course_id,
            err = %e,
            "webhook notification failed"
        ),
    }
}

/// Server酱 (https://sct.ftqq.com)
/// POST https://sctapi.ftqq.com/{SendKey}.send
async fn send_serverchan(
    client: &Client,
    send_key: &str,
    title: &str,
    body: &str,
) -> Result<(), String> {
    let url = format!("https://sctapi.ftqq.com/{send_key}.send");
    debug!(url = %url, "sending Server酱 notification");

    let form = [("title", title), ("desp", body)];
    let res = client
        .post(&url)
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }
    Ok(())
}

/// Custom webhook: POST JSON { "title": "...", "body": "..." }
async fn send_custom(
    client: &Client,
    url: &str,
    title: &str,
    body: &str,
) -> Result<(), String> {
    debug!(url = %url, "sending custom webhook notification");

    #[derive(Serialize)]
    struct Payload<'a> {
        title: &'a str,
        body: &'a str,
    }

    let res = client
        .post(url)
        .json(&Payload { title, body })
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }
    Ok(())
}
