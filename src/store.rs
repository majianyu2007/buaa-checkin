use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tracing::warn;

use crate::error::AppResult;
use crate::webhook::WebhookConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub admin_id: String,
    #[serde(default = "default_poll")]
    pub poll_interval_minutes: u64,
    #[serde(default = "default_window")]
    pub auto_window_minutes: u64,
    #[serde(default)]
    pub students: Vec<StudentEntry>,
    #[serde(default)]
    pub webhook: WebhookConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentEntry {
    pub student_id: String,
    pub name: String,
    #[serde(default)]
    pub course_ids: Vec<String>,
}

fn default_port() -> u16 {
    3000
}

fn default_poll() -> u64 {
    10
}

fn default_window() -> u64 {
    15
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            admin_id: String::new(),
            poll_interval_minutes: default_poll(),
            auto_window_minutes: default_window(),
            students: Vec::new(),
            webhook: WebhookConfig::default(),
        }
    }
}

/// Persistent store backed by a JSON file.
pub struct Store {
    path: PathBuf,
    inner: RwLock<StoreConfig>,
}

impl Store {
    pub fn load<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref().to_path_buf();
        let config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(text) => match serde_json::from_str(&text) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        warn!(err = %e, "failed to parse config, using defaults");
                        StoreConfig::default()
                    }
                },
                Err(e) => {
                    warn!(err = %e, "failed to read config file, using defaults");
                    StoreConfig::default()
                }
            }
        } else {
            StoreConfig::default()
        };

        Self {
            path,
            inner: RwLock::new(config),
        }
    }

    pub fn save(&self) -> AppResult<()> {
        let config = self.inner.read().unwrap();
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(&*config)?;
        std::fs::write(&self.path, text)?;
        Ok(())
    }

    pub fn config(&self) -> StoreConfig {
        self.inner.read().unwrap().clone()
    }

    pub fn port(&self) -> u16 {
        self.inner.read().unwrap().port
    }

    pub fn set_port(&self, port: u16) -> AppResult<()> {
        self.inner.write().unwrap().port = port;
        self.save()
    }

    pub fn admin_id(&self) -> String {
        self.inner.read().unwrap().admin_id.clone()
    }

    pub fn set_admin_id(&self, admin_id: String) -> AppResult<()> {
        self.inner.write().unwrap().admin_id = admin_id;
        self.save()
    }

    pub fn is_admin(&self, student_id: &str) -> bool {
        let config = self.inner.read().unwrap();
        if config.admin_id == student_id {
            return true;
        }
        if config.admin_id.is_empty() {
            if let Some(first) = config.students.first() {
                return first.student_id == student_id;
            }
            return true; // No users yet, the first to login gets admin-like access
        }
        false
    }

    pub fn poll_interval_minutes(&self) -> u64 {
        self.inner.read().unwrap().poll_interval_minutes
    }

    pub fn add_courses(&self, student_id: &str, course_ids: Vec<String>) -> AppResult<()> {
        let mut config = self.inner.write().unwrap();
        if let Some(s) = config.students.iter_mut().find(|s| s.student_id == student_id) {
            for id in course_ids {
                if !s.course_ids.contains(&id) {
                    s.course_ids.push(id);
                }
            }
        } else {
            config.students.push(StudentEntry {
                student_id: student_id.to_string(),
                name: "未知用户".to_string(),
                course_ids,
            });
        }
        Ok(())
    }

    pub fn students(&self) -> Vec<StudentEntry> {
        self.inner.read().unwrap().students.clone()
    }

    pub fn add_student(&self, entry: StudentEntry) -> AppResult<()> {
        {
            let mut config = self.inner.write().unwrap();
            // Upsert: if student_id exists, update it
            if let Some(existing) = config
                .students
                .iter_mut()
                .find(|s| s.student_id == entry.student_id)
            {
                existing.name = entry.name;
                existing.course_ids = entry.course_ids;
            } else {
                config.students.push(entry);
            }
        }
        self.save()
    }

    pub fn remove_student(&self, student_id: &str) -> AppResult<bool> {
        let removed;
        {
            let mut config = self.inner.write().unwrap();
            let before = config.students.len();
            config.students.retain(|s| s.student_id != student_id);
            removed = config.students.len() < before;
        }
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    pub fn get_student(&self, student_id: &str) -> Option<StudentEntry> {
        self.inner
            .read()
            .unwrap()
            .students
            .iter()
            .find(|s| s.student_id == student_id)
            .cloned()
    }

    pub fn webhook(&self) -> WebhookConfig {
        self.inner.read().unwrap().webhook.clone()
    }

    pub fn set_webhook(&self, webhook: WebhookConfig) -> AppResult<()> {
        {
            let mut config = self.inner.write().unwrap();
            config.webhook = webhook;
        }
        self.save()
    }
}
