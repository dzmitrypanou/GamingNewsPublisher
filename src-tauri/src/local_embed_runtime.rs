use crate::models::AppSettings;
use crate::services::{llm_dir, local_model_catalog};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

pub const LOCAL_EMBED_PORT: u16 = 18766;

struct LoadedEmbedConfig {
    model_id: String,
    device: String,
}

pub struct LocalEmbedRuntime {
    child: Mutex<Option<Child>>,
    server_ready: AtomicBool,
    loaded: Mutex<Option<LoadedEmbedConfig>>,
    last_start_error: Mutex<Option<String>>,
}

impl LocalEmbedRuntime {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            server_ready: AtomicBool::new(false),
            loaded: Mutex::new(None),
            last_start_error: Mutex::new(None),
        }
    }

    pub fn last_start_error(&self) -> Option<String> {
        self.last_start_error.lock().ok().and_then(|g| g.clone())
    }

    fn set_start_error(&self, message: Option<String>) {
        if let Ok(mut guard) = self.last_start_error.lock() {
            *guard = message;
        }
    }

    pub fn is_files_ready(&self, model_id: &str) -> bool {
        llm_dir::server_installed() && llm_dir::model_installed(model_id)
    }

    pub fn is_server_running(&self) -> bool {
        self.server_ready.load(Ordering::SeqCst)
    }

    pub fn is_ready(&self, model_id: &str) -> bool {
        self.is_files_ready(model_id) && self.is_server_running()
    }

    pub fn embeddings_url(&self) -> String {
        format!("http://127.0.0.1:{}/v1/embeddings", LOCAL_EMBED_PORT)
    }

    pub fn stop(&self) {
        self.server_ready.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        if let Ok(mut loaded) = self.loaded.lock() {
            *loaded = None;
        }
    }

    pub fn shutdown(&self) {
        self.stop();
    }

    fn config_matches(&self, settings: &AppSettings, model_id: &str) -> bool {
        let Ok(guard) = self.loaded.lock() else {
            return false;
        };
        guard.as_ref().is_some_and(|loaded| {
            loaded.model_id == model_id && loaded.device == settings.local_llm_device
        })
    }

    pub async fn start(&self, settings: &AppSettings, model_id: &str) -> Result<()> {
        if !self.is_files_ready(model_id) {
            if llm_dir::model_file_invalid(model_id) {
                anyhow::bail!(
                    "Файл модели дедупа повреждён или неполный. Удалите и скачайте заново."
                );
            }
            anyhow::bail!("Модель для проверки дублей не установлена");
        }

        if self.is_server_running() && self.config_matches(settings, model_id) {
            self.set_start_error(None);
            return Ok(());
        }

        self.stop();
        tokio::time::sleep(Duration::from_millis(400)).await;

        let def = local_model_catalog::find(model_id).context("Unknown dedup model")?;
        let server = llm_dir::server_path()?;
        let model = llm_dir::model_path_for(model_id)?;
        let bin_dir = llm_dir::bin_dir()?;
        let ngl = local_model_catalog::resolve_ngl_for_model(
            &settings.local_llm_device,
            settings.local_llm_gpu_layers,
            &def,
        );
        let log_path = llm_dir::embed_server_start_log_path()?;
        let stderr_file = File::create(&log_path)
            .with_context(|| format!("Cannot create {}", log_path.display()))?;

        let mut cmd = Command::new(&server);
        cmd.current_dir(&bin_dir)
            .arg("-m")
            .arg(&model)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(LOCAL_EMBED_PORT.to_string())
            .arg("-c")
            .arg("512")
            .arg("-ngl")
            .arg(ngl.to_string())
            .arg("--embedding")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr_file));

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to start embed {}", server.display()))?;

        *self.child.lock().unwrap() = Some(child);
        *self.loaded.lock().unwrap() = Some(LoadedEmbedConfig {
            model_id: model_id.to_string(),
            device: settings.local_llm_device.clone(),
        });

        for _ in 0..40 {
            if let Some(message) = self.child_exit_message() {
                self.stop();
                self.set_start_error(Some(message.clone()));
                anyhow::bail!(message);
            }
            if self.health_check_matches_child().await {
                self.server_ready.store(true, Ordering::SeqCst);
                self.set_start_error(None);
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        self.stop();
        let log_tail = read_log_tail(&log_path, 400);
        let message = if log_tail.is_empty() {
            "embed-server не ответил вовремя".to_string()
        } else {
            format!("embed-server не запустился: {log_tail}")
        };
        self.set_start_error(Some(message.clone()));
        anyhow::bail!(message)
    }

    pub async fn ensure_running(&self, settings: &AppSettings, model_id: &str) -> Result<()> {
        if self.is_server_running() && self.config_matches(settings, model_id) {
            return Ok(());
        }
        self.start(settings, model_id).await
    }

    fn child_exit_message(&self) -> Option<String> {
        let Ok(mut guard) = self.child.lock() else {
            return None;
        };
        let Some(child) = guard.as_mut() else {
            return None;
        };
        match child.try_wait() {
            Ok(Some(_)) => {
                let log_path = llm_dir::embed_server_start_log_path().ok()?;
                let tail = read_log_tail(&log_path, 200);
                if tail.is_empty() {
                    Some("embed-server завершился с ошибкой".into())
                } else {
                    Some(format!("embed-server завершился: {tail}"))
                }
            }
            Ok(None) => None,
            Err(_) => Some("Не удалось проверить состояние embed-server".into()),
        }
    }

    async fn health_check_matches_child(&self) -> bool {
        if !Self::health_check().await {
            return false;
        }
        let Ok(mut guard) = self.child.lock() else {
            return false;
        };
        let Some(child) = guard.as_mut() else {
            return false;
        };
        matches!(child.try_wait(), Ok(None))
    }

    async fn health_check() -> bool {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };
        let health_url = format!("http://127.0.0.1:{}/health", LOCAL_EMBED_PORT);
        client
            .get(&health_url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

fn read_log_tail(path: &std::path::Path, max_chars: usize) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if len > 8192 {
        let _ = file.seek(SeekFrom::End(-8192));
    }
    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        return String::new();
    }
    if buf.len() > max_chars {
        buf[buf.len().saturating_sub(max_chars)..].to_string()
    } else {
        buf
    }
}
