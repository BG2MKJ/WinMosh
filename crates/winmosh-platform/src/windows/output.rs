#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputCapability {
    VirtualTerminal,
    Plain,
}

pub fn detect_output_capability() -> OutputCapability {
    use std::io::IsTerminal;

    if std::io::stdout().is_terminal() {
        OutputCapability::VirtualTerminal
    } else {
        OutputCapability::Plain
    }
}
