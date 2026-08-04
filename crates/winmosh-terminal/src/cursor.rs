#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub column: u16,
    pub row: u16,
    pub visible: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            column: 0,
            row: 0,
            visible: true,
        }
    }
}
