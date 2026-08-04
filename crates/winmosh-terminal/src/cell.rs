use crate::rendition::Rendition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub rendition: Rendition,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            text: " ".to_owned(),
            rendition: Rendition::default(),
        }
    }
}

impl Cell {
    pub fn blank(rendition: Rendition) -> Self {
        Self {
            text: " ".to_owned(),
            rendition,
        }
    }
}
