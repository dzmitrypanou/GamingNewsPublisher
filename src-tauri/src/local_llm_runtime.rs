use crate::models::AppSettings;

use crate::services::{llm_dir, local_model_catalog};

use anyhow::{Context, Result};

use std::collections::HashMap;

use std::fs::File;

use std::io::{Read, Seek, SeekFrom};

use std::process::{Child, Command, Stdio};

use std::sync::atomic::{AtomicBool, Ordering};

use std::sync::{Arc, Mutex};

use std::time::Duration;

pub const LOCAL_LLM_PORT: u16 = 18765;

#[derive(Debug, Clone, Default)]

pub struct DownloadSnapshot {

    pub bytes_done: u64,

    pub bytes_total: u64,

    pub stage: String,

    pub error: Option<String>,

}

struct ActiveDownload {

    snapshot: DownloadSnapshot,

    cancel: Arc<AtomicBool>,

}

#[derive(Default)]

pub struct DownloadRegistry {

    server: Option<ActiveDownload>,

    models: HashMap<String, ActiveDownload>,

}

impl DownloadRegistry {

    pub fn try_start_server(&mut self) -> Option<Arc<AtomicBool>> {

        if self.server.is_some() {

            return None;

        }

        let cancel = Arc::new(AtomicBool::new(false));

        self.server = Some(ActiveDownload {

            snapshot: DownloadSnapshot {

                stage: "starting".into(),

                ..Default::default()

            },

            cancel: cancel.clone(),

        });

        Some(cancel)

    }

    pub fn try_start_model(&mut self, model_id: &str) -> Option<Arc<AtomicBool>> {

        if self.models.contains_key(model_id) {

            return None;

        }

        let cancel = Arc::new(AtomicBool::new(false));

        self.models.insert(

            model_id.to_string(),

            ActiveDownload {

                snapshot: DownloadSnapshot {

                    stage: "starting".into(),

                    ..Default::default()

                },

                cancel: cancel.clone(),

            },

        );

        Some(cancel)

    }

    pub fn update_server(&mut self, snapshot: DownloadSnapshot) {

        if let Some(entry) = &mut self.server {

            entry.snapshot = snapshot;

        }

    }

    pub fn update_model(&mut self, model_id: &str, snapshot: DownloadSnapshot) {

        if let Some(entry) = self.models.get_mut(model_id) {

            entry.snapshot = snapshot;

        }

    }

    pub fn set_server_error(&mut self, error: String) {

        if let Some(entry) = &mut self.server {

            entry.snapshot.stage = "error".into();

            entry.snapshot.error = Some(error);

        }

    }

    pub fn set_model_error(&mut self, model_id: &str, error: String) {

        if let Some(entry) = self.models.get_mut(model_id) {

            entry.snapshot.stage = "error".into();

            entry.snapshot.error = Some(error);

        }

    }

    pub fn finish_server(&mut self) {

        self.server = None;

    }

    pub fn finish_model(&mut self, model_id: &str) {

        self.models.remove(model_id);

    }

    pub fn cancel_server(&mut self) -> bool {

        if let Some(entry) = &self.server {

            entry.cancel.store(true, Ordering::SeqCst);

            true

        } else {

            false

        }

    }

    pub fn cancel_model(&mut self, model_id: &str) -> bool {

        if let Some(entry) = self.models.get(model_id) {

            entry.cancel.store(true, Ordering::SeqCst);

            true

        } else {

            false

        }

    }

    pub fn server_snapshot(&self) -> Option<DownloadSnapshot> {

        self.server.as_ref().map(|e| e.snapshot.clone())

    }

    pub fn model_snapshot(&self, model_id: &str) -> Option<DownloadSnapshot> {

        self.models.get(model_id).map(|e| e.snapshot.clone())

    }

    pub fn any_active(&self) -> bool {

        self.server.is_some() || !self.models.is_empty()

    }

    pub fn is_model_active(&self, model_id: &str) -> bool {

        self.models.contains_key(model_id)

    }

}

pub fn snapshot_progress_pct(snapshot: &DownloadSnapshot) -> f64 {

    if snapshot.bytes_total > 0 {

        (snapshot.bytes_done as f64 / snapshot.bytes_total as f64 * 100.0).min(100.0)

    } else {

        0.0

    }

}

#[derive(Debug, Clone, PartialEq, Eq)]

struct LoadedConfig {

    model_id: String,

    device: String,

    gpu_layers: u32,

}

pub struct LocalLlmRuntime {

    child: Mutex<Option<Child>>,

    server_ready: AtomicBool,

    loaded: Mutex<Option<LoadedConfig>>,

    last_start_error: Mutex<Option<String>>,

    pub downloads: Mutex<DownloadRegistry>,

    server_install: Arc<tokio::sync::Mutex<()>>,

}

impl LocalLlmRuntime {

    pub fn new() -> Self {

        Self {

            child: Mutex::new(None),

            server_ready: AtomicBool::new(false),

            loaded: Mutex::new(None),

            last_start_error: Mutex::new(None),

            downloads: Mutex::new(DownloadRegistry::default()),

            server_install: Arc::new(tokio::sync::Mutex::new(())),

        }

    }

    pub fn server_install_lock(&self) -> Arc<tokio::sync::Mutex<()>> {

        self.server_install.clone()

    }

    pub fn last_start_error(&self) -> Option<String> {

        self.last_start_error.lock().ok().and_then(|g| g.clone())

    }

    fn set_start_error(&self, message: Option<String>) {

        if let Ok(mut guard) = self.last_start_error.lock() {

            *guard = message;

        }

    }

    pub fn is_files_ready_for(&self, model_id: &str) -> bool {
        llm_dir::files_ready(model_id)
    }

    pub fn is_files_ready(&self, settings: &AppSettings) -> bool {
        self.is_files_ready_for(&settings.normalized_local_model_id())
    }

    pub fn is_server_running(&self) -> bool {

        self.server_ready.load(Ordering::SeqCst)

    }

    pub fn is_ready_for_model(&self, settings: &AppSettings, model_id: &str) -> bool {
        self.is_files_ready_for(model_id)
            && self.is_server_running()
            && self.config_matches_model(settings, model_id)
    }

    pub fn is_ready(&self, settings: &AppSettings) -> bool {
        self.is_ready_for_model(settings, &settings.normalized_local_model_id())
    }

    pub fn chat_completions_url(&self) -> String {

        format!("http://127.0.0.1:{}/v1/chat/completions", LOCAL_LLM_PORT)

    }

    pub fn stop(&self) {

        self.server_ready.store(false, Ordering::SeqCst);

        if let Ok(mut guard) = self.child.lock() {

            if let Some(mut child) = guard.take() {

                kill_process_tree(child.id());

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

        kill_orphan_llama_servers();

    }

    pub fn stop_for_install(&self) {

        self.shutdown();

    }

    fn config_matches_model(&self, settings: &AppSettings, model_id: &str) -> bool {
        let Ok(guard) = self.loaded.lock() else {
            return false;
        };
        guard.as_ref().is_some_and(|loaded| {
            loaded.model_id == model_id
                && loaded.device == settings.local_llm_device
                && loaded.gpu_layers == settings.local_llm_gpu_layers
        })
    }

    fn config_matches(&self, settings: &AppSettings) -> bool {
        self.config_matches_model(settings, &settings.normalized_local_model_id())
    }

    pub async fn start(&self, settings: &AppSettings) -> Result<()> {
        self.start_for_model(settings, &settings.normalized_local_model_id())
            .await
    }

    pub async fn start_for_model(&self, settings: &AppSettings, model_id: &str) -> Result<()> {
        if !self.is_files_ready_for(model_id) {
            if llm_dir::model_file_invalid(model_id) {
                anyhow::bail!(
                    "Файл модели повреждён или неполный. Удалите её и скачайте заново."
                );
            }
            anyhow::bail!("Local LLM files not installed");
        }

        if self.is_server_running() && self.config_matches_model(settings, model_id) {
            self.set_start_error(None);
            return Ok(());
        }

        self.stop();
        tokio::time::sleep(Duration::from_millis(400)).await;

        let server = llm_dir::server_path()?;

        let model = llm_dir::model_path_for(model_id)?;

        let bin_dir = llm_dir::bin_dir()?;

        let ngl = settings.active_ngl();

        let log_path = llm_dir::server_start_log_path()?;

        let stderr_file = File::create(&log_path)

            .with_context(|| format!("Cannot create {}", log_path.display()))?;

        let mut cmd = Command::new(&server);

        cmd.current_dir(&bin_dir)

            .arg("-m")

            .arg(&model)

            .arg("--host")

            .arg("127.0.0.1")

            .arg("--port")

            .arg(LOCAL_LLM_PORT.to_string())

            .arg("-c")

            .arg("4096")

            .arg("-ngl")

            .arg(ngl.to_string())

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

            .with_context(|| format!("Failed to start {}", server.display()))?;

        *self.child.lock().unwrap() = Some(child);

        *self.loaded.lock().unwrap() = Some(LoadedConfig {
            model_id: model_id.to_string(),
            device: settings.local_llm_device.clone(),
            gpu_layers: settings.local_llm_gpu_layers,
        });

        let attempts = startup_wait_attempts(model_id);

        for _ in 0..attempts {
            if let Some(message) = self.child_exit_message() {
                self.stop();
                let hint = startup_failure_hint(model_id, settings);

                self.set_start_error(Some(format!("{message}{hint}")));

                anyhow::bail!("{message}{hint}");

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

            "llama-server не ответил вовремя — модель может долго загружаться".to_string()

        } else {

            format!("llama-server не запустился: {log_tail}")

        };

        let hint = startup_failure_hint(model_id, settings);
        let full = format!("{message}{hint}");
        self.set_start_error(Some(full.clone()));
        anyhow::bail!(full)
    }

    pub async fn ensure_running(&self, settings: &AppSettings) -> Result<()> {
        self.ensure_running_for_model(settings, &settings.normalized_local_model_id())
            .await
    }

    pub async fn ensure_running_for_model(
        &self,
        settings: &AppSettings,
        model_id: &str,
    ) -> Result<()> {
        if self.is_server_running() && self.config_matches_model(settings, model_id) {
            return Ok(());
        }
        self.start_for_model(settings, model_id).await
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

                let log_path = llm_dir::server_start_log_path().ok()?;

                let tail = read_log_tail(&log_path, 500);

                if tail.is_empty() {

                    Some("llama-server завершился с ошибкой".to_string())

                } else {

                    Some(format!("llama-server завершился: {tail}"))

                }

            }

            Ok(None) => None,

            Err(_) => Some("Не удалось проверить состояние llama-server".to_string()),

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

        let health_url = format!("http://127.0.0.1:{}/health", LOCAL_LLM_PORT);

        if client

            .get(&health_url)

            .send()

            .await

            .map(|r| r.status().is_success())

            .unwrap_or(false)

        {

            return true;

        }

        let models_url = format!("http://127.0.0.1:{}/v1/models", LOCAL_LLM_PORT);

        client

            .get(&models_url)

            .send()

            .await

            .map(|r| r.status().is_success())

            .unwrap_or(false)

    }

}

impl Drop for LocalLlmRuntime {

    fn drop(&mut self) {

        self.shutdown();

    }

}

fn startup_wait_attempts(model_id: &str) -> u32 {

    let size_gb = local_model_catalog::find(model_id)

        .map(|m| m.size_hint_bytes)

        .unwrap_or(4_500_000_000) as f64

        / 1_073_741_824.0;

    (60.0 + size_gb * 30.0).round() as u32

}

fn startup_failure_hint(model_id: &str, settings: &AppSettings) -> String {

    let mut hints = Vec::new();

    if let Some(def) = local_model_catalog::find(model_id) {

        if def.min_vram_gb >= 8 && settings.local_llm_device == "gpu" {

            hints.push(" На 8 GB VRAM попробуйте режим «Гибрид» (~24 слоя) или CPU.");

        }

    }

    if llm_dir::model_file_invalid(model_id) {

        hints.push(" Файл модели повреждён — удалите и скачайте заново.");

    }

    hints.concat()

}

fn read_log_tail(path: &std::path::Path, max_chars: usize) -> String {

    let Ok(mut file) = File::open(path) else {

        return String::new();

    };

    let Ok(meta) = file.metadata() else {

        return String::new();

    };

    let read_from = meta.len().saturating_sub(16_384);

    if file.seek(SeekFrom::Start(read_from)).is_err() {

        return String::new();

    }

    let mut buffer = String::new();

    if file.read_to_string(&mut buffer).is_err() {

        return String::new();

    }

    let compact: String = buffer

        .lines()

        .filter(|line| {

            !line.is_empty()

                && !line.starts_with("ggml_vulkan:")

                && !line.starts_with("build:")

                && !line.starts_with("system info:")

                && !line.starts_with("system_info:")

        })

        .collect::<Vec<_>>()

        .join(" ")

        .trim()

        .to_string();

    if compact.chars().count() <= max_chars {

        compact

    } else {

        compact

            .chars()

            .rev()

            .take(max_chars)

            .collect::<String>()

            .chars()

            .rev()

            .collect()

    }

}

#[cfg(windows)]

fn kill_process_tree(pid: u32) {

    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    if pid == 0 {

        return;

    }

    let _ = Command::new("taskkill")

        .args(["/F", "/T", "/PID", &pid.to_string()])

        .creation_flags(CREATE_NO_WINDOW)

        .status();

}

#[cfg(not(windows))]

fn kill_process_tree(_pid: u32) {}

#[cfg(windows)]

fn kill_orphan_llama_servers() {

    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let _ = Command::new("taskkill")

        .args(["/F", "/IM", "llama-server.exe", "/T"])

        .creation_flags(CREATE_NO_WINDOW)

        .status();

}

#[cfg(not(windows))]

fn kill_orphan_llama_servers() {}

