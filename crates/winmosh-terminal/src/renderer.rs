use crate::framebuffer::Framebuffer;
use crate::rendition::{Color, Intensity, Rendition};

pub fn render_diff(previous: Option<&Framebuffer>, current: &Framebuffer) -> String {
    let Some(prev) = previous else {
        return render_framebuffer(current);
    };
    if prev.size.columns != current.size.columns || prev.size.rows != current.size.rows {
        return render_framebuffer(current);
    }

    let mut output = String::new();
    let mut last_rendition = Rendition::default();

    let cols = current.size.columns;
    let rows = current.size.rows;
    for row in 0..rows {
        for col in 0..cols {
            let p = prev.cell(col, row);
            let c = current.cell(col, row);
            if p == c {
                continue;
            }
            if let Some(cell) = c {
                if cell.rendition != last_rendition {
                    output.push_str(&rendition_sequence(cell.rendition));
                    last_rendition = cell.rendition;
                }
                output.push_str(&format!("\x1b[{};{}H", row + 1, col + 1));
                output.push_str(&cell.text);
            }
        }
    }

    if current.cursor != prev.cursor || current.cursor.visible != prev.cursor.visible {
        output.push_str(&format!(
            "\x1b[{};{}H",
            current.cursor.row + 1,
            current.cursor.column + 1
        ));
        if current.cursor.visible {
            output.push_str("\x1b[?25h");
        } else {
            output.push_str("\x1b[?25l");
        }
    }
    output
}

pub fn render_framebuffer(framebuffer: &Framebuffer) -> String {
    let mut output = String::from("\x1b[H\x1b[2J");
    for row in 0..framebuffer.size.rows {
        if row > 0 {
            output.push_str("\r\n");
        }
        if let Some(cells) = framebuffer.row(row) {
            let mut last_rendition = Rendition::default();
            for cell in cells {
                if cell.rendition != last_rendition {
                    output.push_str(&rendition_sequence(cell.rendition));
                    last_rendition = cell.rendition;
                }
                output.push_str(&cell.text);
            }
        }
    }
    output.push_str(&format!(
        "\x1b[{};{}H",
        framebuffer.cursor.row + 1,
        framebuffer.cursor.column + 1
    ));
    if framebuffer.cursor.visible {
        output.push_str("\x1b[?25h");
    } else {
        output.push_str("\x1b[?25l");
    }
    output
}

pub fn rendition_sequence(rendition: Rendition) -> String {
    let mut codes = vec![0_u16];
    if rendition.intensity == Intensity::Bold {
        codes.push(1);
    }
    if rendition.underline {
        codes.push(4);
    }
    if rendition.inverse {
        codes.push(7);
    }
    if let Color::Indexed(value) = rendition.foreground {
        codes.push(38);
        codes.push(5);
        codes.push(u16::from(value));
    }
    if let Color::Indexed(value) = rendition.background {
        codes.push(48);
        codes.push(5);
        codes.push(u16::from(value));
    }
    let parameters = codes
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(";");
    format!("\x1b[{parameters}m")
}

#[cfg(test)]
mod tests {
    use super::render_diff;
    use crate::framebuffer::Framebuffer;

    #[test]
    fn renders_cursor_and_text() {
        let mut fb = Framebuffer::new(3, 1);
        fb.put("x".to_owned());
        let rendered = render_diff(None, &fb);
        assert!(rendered.contains('x'));
        assert!(rendered.contains("[1;2H"));
    }

    #[test]
    fn diff_only_outputs_changes() {
        let mut prev = Framebuffer::new(5, 2);
        prev.put("a".to_owned());
        let mut curr = prev.clone();
        curr.put("b".to_owned());
        curr.put("c".to_owned());
        let diff = render_diff(Some(&prev), &curr);
        assert!(!diff.contains('a'), "should not contain unchanged cell");
        assert!(diff.contains('b'), "should contain new cell");
    }
}
