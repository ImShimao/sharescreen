//! Wrapper autour d'adb.exe (platform-tools de Google). Arguments passés sans shell.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tokio::process::{Child, Command};
use tokio::time::timeout;

use crate::paths;

/// Pas de fenêtre de console pour les processus enfants.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
pub struct AdbDevice {
    pub serial: String,
    pub state: String,
    pub model: String,
}

fn exe() -> PathBuf {
    paths::bin("adb.exe")
}

fn command(args: &[&str]) -> Command {
    let mut cmd = Command::new(exe());
    cmd.args(args).stdin(Stdio::null());
    #[cfg(windows)]
    cmd.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    cmd
}

async fn run(args: &[&str], seconds: u64) -> Result<Vec<u8>, String> {
    let out = timeout(Duration::from_secs(seconds), command(args).output())
        .await
        .map_err(|_| "adb ne répond pas (délai dépassé).".to_string())?
        .map_err(|e| format!("adb introuvable : {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.is_empty() {
            Err(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(err)
        }
    }
}

pub async fn list_devices() -> Result<Vec<AdbDevice>, String> {
    let out = String::from_utf8_lossy(&run(&["devices", "-l"], 15).await?).into_owned();
    let mut devices = Vec::new();
    for line in out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 || parts[0] == "List" {
            continue;
        }
        let model = parts
            .iter()
            .find_map(|p| p.strip_prefix("model:"))
            .map(|m| m.replace('_', " "))
            .unwrap_or_default();
        devices.push(AdbDevice { serial: parts[0].to_string(), state: parts[1].to_string(), model });
    }
    Ok(devices)
}

pub async fn getprop(serial: &str, key: &str) -> Result<String, String> {
    let out = run(&["-s", serial, "shell", "getprop", key], 10).await?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

pub async fn forward(serial: &str, port: u16, socket: &str) -> Result<(), String> {
    let local = format!("tcp:{port}");
    let remote = format!("localabstract:{socket}");
    run(&["-s", serial, "forward", &local, &remote], 15).await.map(|_| ())
}

pub async fn remove_forward(serial: &str, port: u16) {
    let local = format!("tcp:{port}");
    let _ = run(&["-s", serial, "forward", "--remove", &local], 10).await;
}

pub async fn push(serial: &str, local: &str, remote: &str) -> Result<(), String> {
    run(&["-s", serial, "push", local, remote], 600).await.map(|_| ())
}

pub async fn pull(serial: &str, remote: &str, local: &str) -> Result<(), String> {
    run(&["-s", serial, "pull", remote, local], 600).await.map(|_| ())
}

/// Contenu d'un dossier de l'appareil : noms, dossiers suffixés par "/".
pub async fn list_dir(serial: &str, dir: &str) -> Result<Vec<String>, String> {
    let out = String::from_utf8_lossy(&run(&["-s", serial, "shell", "ls", "-1p", dir], 15).await?).into_owned();
    Ok(out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with("ls:"))
        .collect())
}

/// Lance une commande shell longue durée sur l'appareil (serveur scrcpy).
pub fn spawn_shell(serial: &str, command_line: &[String]) -> Result<Child, String> {
    let mut cmd = Command::new(exe());
    cmd.arg("-s").arg(serial).arg("shell").args(command_line);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(|e| format!("Impossible de lancer adb : {e}"))
}
