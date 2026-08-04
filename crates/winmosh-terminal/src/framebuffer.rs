use crate::cell::Cell;
use crate::cursor::Cursor;
use crate::rendition::Rendition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramebufferSize {
    pub columns: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    pub size: FramebufferSize,
    pub cursor: Cursor,
    pub rendition: Rendition,
    cells: Vec<Cell>,
    wrap_pending: bool,
}

impl Framebuffer {
    pub fn new(columns: u16, rows: u16) -> Self {
        let size = FramebufferSize { columns, rows };
        Self {
            size,
            cursor: Cursor::default(),
            rendition: Rendition::default(),
            cells: vec![Cell::default(); usize::from(columns) * usize::from(rows)],
            wrap_pending: false,
        }
    }

    pub fn resize(&mut self, columns: u16, rows: u16) {
        let old = self.clone();
        *self = Self::new(columns, rows);
        self.rendition = old.rendition;
        self.cursor.visible = old.cursor.visible;
        let copy_columns = columns.min(old.size.columns);
        let copy_rows = rows.min(old.size.rows);
        for row in 0..copy_rows {
            for column in 0..copy_columns {
                let old_index = old.index(column, row);
                let new_index = self.index(column, row);
                self.cells[new_index] = old.cells[old_index].clone();
            }
        }
        self.cursor.column = self.cursor.column.min(columns.saturating_sub(1));
        self.cursor.row = self.cursor.row.min(rows.saturating_sub(1));
    }

    pub fn clear(&mut self) {
        self.cells.fill(Cell::blank(self.rendition));
        self.cursor = Cursor {
            column: 0,
            row: 0,
            visible: self.cursor.visible,
        };
        self.wrap_pending = false;
    }

    pub fn clear_line_from_cursor(&mut self) {
        for column in self.cursor.column..self.size.columns {
            let index = self.index(column, self.cursor.row);
            self.cells[index] = Cell::blank(self.rendition);
        }
    }

    pub fn clear_line_to_cursor(&mut self) {
        for column in 0..=self.cursor.column.min(self.size.columns.saturating_sub(1)) {
            let index = self.index(column, self.cursor.row);
            self.cells[index] = Cell::blank(self.rendition);
        }
    }

    pub fn clear_line(&mut self) {
        for column in 0..self.size.columns {
            let index = self.index(column, self.cursor.row);
            self.cells[index] = Cell::blank(self.rendition);
        }
    }

    pub fn clear_screen_from_cursor(&mut self) {
        self.clear_line_from_cursor();
        for row in (self.cursor.row + 1)..self.size.rows {
            for column in 0..self.size.columns {
                let index = self.index(column, row);
                self.cells[index] = Cell::blank(self.rendition);
            }
        }
    }

    pub fn clear_screen_to_cursor(&mut self) {
        for row in 0..self.cursor.row {
            for column in 0..self.size.columns {
                let index = self.index(column, row);
                self.cells[index] = Cell::blank(self.rendition);
            }
        }
        self.clear_line_to_cursor();
    }

    pub fn scroll_up(&mut self, lines: u16) {
        let lines = lines.min(self.size.rows);
        let columns = usize::from(self.size.columns);
        let rows = usize::from(self.size.rows);
        let offset = usize::from(lines) * columns;
        if offset >= self.cells.len() {
            self.cells.fill(Cell::blank(self.rendition));
        } else {
            self.cells.rotate_left(offset);
            let start = (rows - usize::from(lines)) * columns;
            self.cells[start..].fill(Cell::blank(self.rendition));
        }
    }

    pub fn put(&mut self, text: String) {
        if self.size.columns == 0 || self.size.rows == 0 {
            return;
        }
        if self.wrap_pending {
            self.cursor.column = 0;
            self.line_feed();
        }
        let index = self.index(self.cursor.column, self.cursor.row);
        self.cells[index] = Cell {
            text,
            rendition: self.rendition,
        };
        self.advance_column();
    }

    pub fn cell(&self, column: u16, row: u16) -> Option<&Cell> {
        (column < self.size.columns && row < self.size.rows)
            .then(|| &self.cells[self.index(column, row)])
    }

    pub fn row(&self, row: u16) -> Option<&[Cell]> {
        if row >= self.size.rows {
            return None;
        }
        let columns = usize::from(self.size.columns);
        let start = usize::from(row) * columns;
        Some(&self.cells[start..start + columns])
    }

    pub fn set_cursor(&mut self, column: u16, row: u16) {
        self.cursor.column = column.min(self.size.columns.saturating_sub(1));
        self.cursor.row = row.min(self.size.rows.saturating_sub(1));
        self.wrap_pending = false;
    }

    pub fn advance_column(&mut self) {
        if self.cursor.column + 1 >= self.size.columns {
            self.cursor.column = self.size.columns.saturating_sub(1);
            self.wrap_pending = true;
        } else {
            self.cursor.column += 1;
        }
    }

    pub fn line_feed(&mut self) {
        self.wrap_pending = false;
        if self.cursor.row + 1 >= self.size.rows {
            self.scroll_up(1);
        } else {
            self.cursor.row += 1;
        }
    }

    pub fn backspace(&mut self) {
        self.wrap_pending = false;
        self.cursor.column = self.cursor.column.saturating_sub(1);
    }

    pub fn insert_lines(&mut self, count: u16) {
        let count = count.min(self.size.rows.saturating_sub(self.cursor.row));
        if count == 0 {
            return;
        }
        let columns = usize::from(self.size.columns);
        let start = usize::from(self.cursor.row) * columns;
        let end = usize::from(self.size.rows) * columns;
        let shift = usize::from(count) * columns;
        for idx in (start..end - shift).rev() {
            let src = self.cells[idx].clone();
            self.cells[idx + shift] = src;
        }
        self.cells[start..start + shift].fill(Cell::blank(self.rendition));
    }

    pub fn delete_lines(&mut self, count: u16) {
        let count = count.min(self.size.rows.saturating_sub(self.cursor.row));
        if count == 0 {
            return;
        }
        let columns = usize::from(self.size.columns);
        let start = usize::from(self.cursor.row) * columns;
        let end = usize::from(self.size.rows) * columns;
        let shift = usize::from(count) * columns;
        for idx in start + shift..end {
            let src = self.cells[idx].clone();
            self.cells[idx - shift] = src;
        }
        let blank_start = end.saturating_sub(shift);
        self.cells[blank_start..end].fill(Cell::blank(self.rendition));
    }

    pub fn erase_characters(&mut self, count: u16) {
        let row = self.cursor.row;
        let start = self.cursor.column;
        let end = (start + count).min(self.size.columns);
        for column in start..end {
            let index = self.index(column, row);
            self.cells[index] = Cell::blank(self.rendition);
        }
    }

    fn index(&self, column: u16, row: u16) -> usize {
        usize::from(row) * usize::from(self.size.columns) + usize::from(column)
    }
}

#[cfg(test)]
mod tests {
    use super::Framebuffer;

    #[test]
    fn scrolls_and_preserves_dimensions() {
        let mut framebuffer = Framebuffer::new(3, 2);
        framebuffer.put("a".to_owned());
        framebuffer.put("b".to_owned());
        framebuffer.put("c".to_owned());
        framebuffer.put("d".to_owned());
        framebuffer.put("e".to_owned());
        framebuffer.put("f".to_owned());
        framebuffer.put("g".to_owned());
        assert_eq!(framebuffer.size.columns, 3);
        assert_eq!(framebuffer.size.rows, 2);
        assert_eq!(
            framebuffer.cell(0, 0).map(|cell| cell.text.as_str()),
            Some("d")
        );
        assert_eq!(
            framebuffer.cell(0, 1).map(|cell| cell.text.as_str()),
            Some("g")
        );
    }
}
