pub mod filesystem;
pub mod process;

#[cfg(windows)]
pub mod windows;

#[cfg(not(windows))]
pub mod windows {
    pub mod console;
    pub mod input;
    pub mod output;
    pub mod resize;
    pub mod signals;
}
