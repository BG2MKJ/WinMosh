#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputCapability {
    Console,
    StdinPipe,
    Unknown,
}

pub fn detect_input_capability() -> InputCapability {
    use std::io::IsTerminal;

    if std::io::stdin().is_terminal() {
        InputCapability::Console
    } else {
        InputCapability::StdinPipe
    }
}
