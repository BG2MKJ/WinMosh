#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intensity {
    Normal,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Default,
    Indexed(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rendition {
    pub intensity: Intensity,
    pub foreground: Color,
    pub background: Color,
    pub underline: bool,
    pub inverse: bool,
}

impl Default for Rendition {
    fn default() -> Self {
        Self {
            intensity: Intensity::Normal,
            foreground: Color::Default,
            background: Color::Default,
            underline: false,
            inverse: false,
        }
    }
}
