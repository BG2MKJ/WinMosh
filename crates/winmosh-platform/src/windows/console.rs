#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::io;

    use super::ConsoleError;

    type Handle = *mut c_void;

    const STD_INPUT_HANDLE: u32 = 0xffff_fff6;
    const STD_OUTPUT_HANDLE: u32 = 0xffff_fff5;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

    extern "system" {
        fn GetStdHandle(std_handle: u32) -> Handle;
        fn GetConsoleMode(console_handle: Handle, mode: *mut u32) -> i32;
        fn SetConsoleMode(console_handle: Handle, mode: u32) -> i32;
    }

    #[derive(Debug)]
    pub struct ConsoleGuard {
        input_handle: Handle,
        output_handle: Handle,
        input_mode: Option<u32>,
        output_mode: Option<u32>,
        active: bool,
    }

    impl ConsoleGuard {
        pub fn enter() -> Result<Self, ConsoleError> {
            let input_handle = std_handle(STD_INPUT_HANDLE)?;
            let output_handle = std_handle(STD_OUTPUT_HANDLE)?;
            let input_mode = get_console_mode(input_handle).ok();
            let output_mode = get_console_mode(output_handle).ok();

            let mut guard = Self {
                input_handle,
                output_handle,
                input_mode,
                output_mode,
                active: false,
            };

            if let Some(mode) = output_mode {
                if let Err(error) = set_console_mode(output_handle, enable_vt_output(mode)) {
                    guard.restore();
                    return Err(error);
                }
            }

            guard.active = true;
            Ok(guard)
        }

        pub fn is_active(&self) -> bool {
            self.active
        }

        fn restore(&mut self) {
            if let Some(mode) = self.input_mode {
                let _ = set_console_mode(self.input_handle, mode);
            }
            if let Some(mode) = self.output_mode {
                let _ = set_console_mode(self.output_handle, mode);
            }
            self.active = false;
        }
    }

    impl Drop for ConsoleGuard {
        fn drop(&mut self) {
            self.restore();
        }
    }

    fn std_handle(kind: u32) -> Result<Handle, ConsoleError> {
        let handle = unsafe { GetStdHandle(kind) };
        if handle == invalid_handle_value() || handle.is_null() {
            Err(ConsoleError::new(format!(
                "GetStdHandle({kind}) failed: {}",
                io::Error::last_os_error()
            )))
        } else {
            Ok(handle)
        }
    }

    fn invalid_handle_value() -> Handle {
        usize::MAX as Handle
    }

    fn get_console_mode(handle: Handle) -> Result<u32, ConsoleError> {
        let mut mode = 0;
        let result = unsafe { GetConsoleMode(handle, &mut mode) };
        if result == 0 {
            Err(ConsoleError::new(format!(
                "GetConsoleMode failed: {}",
                io::Error::last_os_error()
            )))
        } else {
            Ok(mode)
        }
    }

    fn set_console_mode(handle: Handle, mode: u32) -> Result<(), ConsoleError> {
        let result = unsafe { SetConsoleMode(handle, mode) };
        if result == 0 {
            Err(ConsoleError::new(format!(
                "SetConsoleMode failed: {}",
                io::Error::last_os_error()
            )))
        } else {
            Ok(())
        }
    }

    pub fn enable_vt_output(mode: u32) -> u32 {
        mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING
    }
}

#[cfg(not(windows))]
mod imp {
    use super::ConsoleError;

    #[derive(Debug)]
    pub struct ConsoleGuard {
        active: bool,
    }

    impl ConsoleGuard {
        pub fn enter() -> Result<Self, ConsoleError> {
            Ok(Self { active: true })
        }

        pub fn is_active(&self) -> bool {
            self.active
        }
    }

    impl Drop for ConsoleGuard {
        fn drop(&mut self) {
            self.active = false;
        }
    }

    pub fn enable_vt_output(mode: u32) -> u32 {
        mode | 0x0004
    }
}

pub use imp::ConsoleGuard;

pub fn enable_vt_output(mode: u32) -> u32 {
    imp::enable_vt_output(mode)
}

#[derive(Debug)]
pub struct ConsoleError {
    message: String,
}

impl ConsoleError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ConsoleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for ConsoleError {}

#[cfg(test)]
mod tests {
    use super::enable_vt_output;

    #[test]
    fn vt_output_flag_is_added() {
        assert_eq!(enable_vt_output(0x0001), 0x0005);
    }
}
