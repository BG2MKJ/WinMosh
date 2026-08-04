use crate::framebuffer::Framebuffer;
use crate::rendition::{Color, Intensity, Rendition};

pub fn render_framebuffer(framebuffer: &Framebuffer) -> String {
    let mut output = String::from("\x1b[H\x1b[2J");
    for row in 0..framebuffer.size.rows {
        if row > 0 {
            output.push_str("\r\n");
        }
        if let Some(cells) = framebuffer.row(row) {
            for cell in cells {
                output.push_str(&rendition_sequence(cell.rendition));
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

fn rendition_sequence(rendition: Rendition) -> String {
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
    use super::render_framebuffer;
    use crate::framebuffer::Framebuffer;

    #[test]
    fn renders_cursor_and_text() {
        let mut framebuffer = Framebuffer::new(3, 1);
        framebuffer.put("x".to_owned());
        let rendered = render_framebuffer(&framebuffer);
        assert!(rendered.contains('x'));
        assert!(rendered.contains("[1;2H"));
    }
}
