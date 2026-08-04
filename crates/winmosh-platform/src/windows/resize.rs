#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub columns: u16,
    pub rows: u16,
}

pub fn current_terminal_size() -> Option<TerminalSize> {
    platform_current_terminal_size().or_else(read_terminal_size_from_env)
}

fn read_terminal_size_from_env() -> Option<TerminalSize> {
    let columns = std::env::var("COLUMNS").ok()?.trim().parse::<u16>().ok()?;
    let rows = std::env::var("LINES").ok()?.trim().parse::<u16>().ok()?;
    Some(TerminalSize { columns, rows })
}

#[cfg(windows)]
fn platform_current_terminal_size() -> Option<TerminalSize> {
    use std::ffi::c_void;

    type Handle = *mut c_void;

    const STD_OUTPUT_HANDLE: u32 = 0xffff_fff5;

    #[repr(C)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    struct SmallRect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }

    #[repr(C)]
    struct ConsoleScreenBufferInfo {
        size: Coord,
        cursor_position: Coord,
        attributes: u16,
        window: SmallRect,
        maximum_window_size: Coord,
    }

    extern "system" {
        fn GetStdHandle(std_handle: u32) -> Handle;
        fn GetConsoleScreenBufferInfo(
            console_output: Handle,
            info: *mut ConsoleScreenBufferInfo,
        ) -> i32;
    }

    let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if handle.is_null() || handle == usize::MAX as Handle {
        return None;
    }

    let mut info = ConsoleScreenBufferInfo {
        size: Coord { x: 0, y: 0 },
        cursor_position: Coord { x: 0, y: 0 },
        attributes: 0,
        window: SmallRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        maximum_window_size: Coord { x: 0, y: 0 },
    };
    let result = unsafe { GetConsoleScreenBufferInfo(handle, &mut info) };
    if result == 0 {
        return None;
    }

    let columns = i32::from(info.window.right) - i32::from(info.window.left) + 1;
    let rows = i32::from(info.window.bottom) - i32::from(info.window.top) + 1;
    let columns = u16::try_from(columns.max(0)).ok()?;
    let rows = u16::try_from(rows.max(0)).ok()?;
    Some(TerminalSize { columns, rows })
}

#[cfg(not(windows))]
fn platform_current_terminal_size() -> Option<TerminalSize> {
    None
}
