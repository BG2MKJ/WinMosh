use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::error::{Error, Result};

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
    if input.trim().to_ascii_lowercase() != "y" && input.trim().to_ascii_lowercase() != "yes" {
        println!("cancelled");
        return Ok(());
    }

    let mut errors = 0;
    for path in [&exe_path, &wm_path] {
        if path.exists() {
            if let Err(e) = fs::remove_file(path) {
                eprintln!("  failed to remove {}: {e}", path.display());
                errors += 1;
            }
        }
    }
    if config_path.exists() {
        if let Err(e) = fs::remove_file(&config_path) {
            eprintln!("  failed to remove {}: {e}", config_path.display());
            errors += 1;
        }
    }

    remove_path_entry(&install_dir);

    let _ = fs::remove_dir(&install_dir);
    let _ = fs::remove_dir(config_directory());

    if errors == 0 {
        println!("WinMosh removed successfully.");
        println!("Restart your terminal for PATH changes to take effect.");
        Ok(())
    } else {
        Err(Error::Cli(format!("uninstall completed with {errors} error(s)")))
    }
}

fn remove_path_entry(dir: &PathBuf) {
    let dir_str = dir.to_string_lossy();
    let script = format!(
        "$path = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         if ($path -like '*{}*') {{ \
           $path = ($path -split ';' | Where-{{ $_ -ne '{}' }}) -join ';'; \
           [Environment]::SetEnvironmentVariable('Path', $path, 'User') \
         }}",
        dir_str.replace('\'', "''"),
        dir_str.replace('\'', "''"),
    );
    let _ = std::process::Command::new("powershell.exe")
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
