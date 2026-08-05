use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::error::Result;

pub fn run() -> Result<()> {
    let install_dir = install_directory();
    let exe_path = install_dir.join("winmosh.exe");
    let wm_path = install_dir.join("wm.exe");
    let config_path = config_directory().join("config.toml");

    println!("WinMosh Uninstaller");
    println!("====================");
    println!();
    println!("This will remove:");
    if exe_path.exists() {
        println!("  {}", exe_path.display());
    }
    if wm_path.exists() {
        println!("  {}", wm_path.display());
    }
    if config_path.exists() {
        println!("  {}", config_path.display());
    }
    println!("  PATH entry pointing to the install directory");
    println!();
    print!("Proceed? [y/N] ");
    io::stdout().flush().ok();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
        println!("cancelled");
        return Ok(());
    }

    let current = std::env::current_exe().ok();
    let mut pending = Vec::new();

    for path in [&exe_path, &wm_path] {
        if !path.exists() {
            continue;
        }
        let is_self = current
            .as_ref()
            .is_some_and(|c| fs::canonicalize(c).ok() == fs::canonicalize(path).ok());
        if is_self {
            pending.push(path.to_string_lossy().replace('\'', "''"));
        } else {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                    pending.push(path.to_string_lossy().replace('\'', "''"));
                }
                Err(e) => {
                    eprintln!("  failed to remove {}: {e}", path.display());
                }
            }
        }
    }

    if config_path.exists() {
        let _ = fs::remove_file(&config_path);
    }

    remove_path_entry(&install_dir);

    let _ = fs::remove_dir(&install_dir);
    let _ = fs::remove_dir(config_directory());

    if !pending.is_empty() {
        let files_to_remove = pending
            .iter()
            .map(|p| {
                format!(
                    "Remove-Item -Force -LiteralPath '{}' -ErrorAction SilentlyContinue",
                    p
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let dirs = format!(
            "Remove-Item -Recurse -Force -LiteralPath '{}' -ErrorAction SilentlyContinue; \
             Remove-Item -Recurse -Force -LiteralPath '{}' -ErrorAction SilentlyContinue",
            install_dir.to_string_lossy().replace('\'', "''"),
            config_directory().to_string_lossy().replace('\'', "''"),
        );
        let script = format!(
            "$ErrorActionPreference='SilentlyContinue'; \
             Start-Sleep -Seconds 3; \
             {files_to_remove}; \
             {dirs}",
        );
        let tmp = std::env::temp_dir().join("winmosh-cleanup.ps1");
        fs::write(&tmp, script.as_bytes()).ok();
        Command::new("powershell.exe")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-File"])
            .arg(&tmp)
            .spawn()
            .ok();
        println!("Some files are in use. They will be removed when this terminal is closed.");
    }

    println!("WinMosh removed successfully.");
    println!("Restart your terminal for PATH changes to take effect.");
    Ok(())
}

fn remove_path_entry(dir: &std::path::Path) {
    let escaped = dir.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$path = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         if ($path -like '*{0}*') {{ \
           $path = ($path -split ';' | Where-{{ $_ -ne '{0}' }}) -join ';'; \
           [Environment]::SetEnvironmentVariable('Path', $path, 'User') \
         }}",
        escaped
    );
    let _ = Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &script])
        .output();
}

fn install_directory() -> PathBuf {
    std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("WinMosh")
}

fn config_directory() -> PathBuf {
    std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("WinMosh")
}
