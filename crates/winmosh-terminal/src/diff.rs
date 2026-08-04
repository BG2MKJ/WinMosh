use crate::cell::Cell;
use crate::cursor::Cursor;
use crate::framebuffer::Framebuffer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellUpdate {
    pub column: u16,
    pub row: u16,
    pub cell: Cell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalDiff {
    pub changed_cells: usize,
    pub updates: Vec<CellUpdate>,
    pub cursor: Cursor,
}

pub fn diff_framebuffers(previous: &Framebuffer, current: &Framebuffer) -> TerminalDiff {
    let mut updates = Vec::new();
    let columns = previous.size.columns.min(current.size.columns);
    let rows = previous.size.rows.min(current.size.rows);
    for row in 0..rows {
        for column in 0..columns {
            let old_cell = previous.cell(column, row);
            let new_cell = current.cell(column, row);
            if old_cell != new_cell {
                if let Some(cell) = new_cell {
                    updates.push(CellUpdate {
                        column,
                        row,
                        cell: cell.clone(),
                    });
                }
            }
        }
    }
    TerminalDiff {
        changed_cells: updates.len(),
        updates,
        cursor: current.cursor,
    }
}

#[cfg(test)]
mod tests {
    use super::diff_framebuffers;
    use crate::framebuffer::Framebuffer;

    #[test]
    fn reports_changed_cells() {
        let old = Framebuffer::new(2, 1);
        let mut new = old.clone();
        new.put("x".to_owned());
        let diff = diff_framebuffers(&old, &new);
        assert_eq!(diff.changed_cells, 1);
        assert_eq!(diff.updates[0].cell.text, "x");
    }
}
