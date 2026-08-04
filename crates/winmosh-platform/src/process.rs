use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub fn path_entries() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default()
}

pub fn pathext_entries() -> Vec<OsString> {
    if cfg!(windows) {
        std::env::var_os("PATHEXT")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(';')
                    .filter(|entry| !entry.is_empty())
                    .map(OsString::from)
                    .collect()
            })
            .unwrap_or_else(|| vec![OsString::from(".EXE")])
    } else {
        vec![OsString::new()]
    }
}

pub fn find_in_path(command: &str) -> Option<PathBuf> {
    let command_path = Path::new(command);
    if command_path.components().count() > 1 && command_path.exists() {
        return Some(command_path.to_path_buf());
    }

    let extensions = pathext_entries();
    for directory in path_entries() {
        let base = directory.join(command);
        if base.is_file() {
            return Some(base);
        }

        if cfg!(windows) && Path::new(command).extension().is_none() {
            for extension in &extensions {
                let mut candidate = base.clone().into_os_string();
                candidate.push(extension);
                let candidate = PathBuf::from(candidate);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}
