#![windows_subsystem = "windows"]

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    if let Err(err) = launch() {
        log_error(&err);
    }
}

fn launch() -> Result<(), String> {
    let launcher_dir = env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Cannot resolve launcher directory")?
        .to_path_buf();

    let app_exe = launcher_dir.join("app").join("gaming-news-publisher.exe");
    if !app_exe.exists() {
        return Err(format!("Application not found: {}", app_exe.display()));
    }

    Command::new(&app_exe)
        .current_dir(app_exe.parent().unwrap())
        .spawn()
        .map_err(|e| format!("Failed to start app: {}", e))?;

    Ok(())
}

fn log_error(message: &str) {
    let log_path = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("launcher-error.log");

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{}", message);
    }
}
