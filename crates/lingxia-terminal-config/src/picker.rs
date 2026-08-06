//! Choosing a theme with the terminal itself as the preview.
//!
//! Colors are previewed by emitting the standard palette sequences (OSC 4,
//! 10, 11) into the session the picker is running in. That is not a private
//! channel we invented: every terminal has implemented these for decades, so
//! nothing new is exposed, and it beats the alternatives on every axis that
//! matters here.
//!
//! It is instant, because it travels in-band with the output rather than
//! through the filesystem. It is scoped to this session, which is what a
//! preview should be — writing the configuration file would repaint every
//! open window while you arrow through a list. And it persists nothing, so
//! abandoning the picker cannot leave a scheme nobody chose in the
//! configuration.
//!
//! Only the committed choice is written to disk.

use crate::ThemeStore;
use lingxia_terminal::TerminalTheme;
use std::io::Write;

/// What the picker returned.
pub enum Choice {
    /// Commit this name.
    Selected(String),
    /// Leave everything as it was.
    Cancelled,
}

/// Present `names`, previewing each as it is highlighted.
///
/// `current` is restored on cancel — by re-emitting its colors rather than
/// asking the terminal to reset, so the session lands exactly where it
/// started regardless of what the terminal considers its default.
pub fn pick_theme(
    store: &ThemeStore,
    names: &[String],
    current: &TerminalTheme,
    current_name: &str,
) -> std::io::Result<Choice> {
    if names.is_empty() {
        return Ok(Choice::Cancelled);
    }
    let mut index = names
        .iter()
        .position(|name| name == current_name)
        .unwrap_or(0);

    let mut terminal = RawTerminal::enter()?;
    let mut out = std::io::stdout();
    writeln!(out, "\r\n  ↑↓ preview   ⏎ keep   esc cancel\r")?;
    let mut drawn = 0usize;

    loop {
        // Redraw in place: the list is short and this keeps the terminal's
        // own scrollback free of a flickering menu.
        if drawn > 0 {
            write!(out, "\x1b[{drawn}A")?;
        }
        drawn = 0;
        for (position, name) in names.iter().enumerate() {
            let marker = if position == index { "▸" } else { " " };
            write!(out, "\x1b[2K  {marker} {name}\r\n")?;
            drawn += 1;
        }
        out.flush()?;

        if let Some(theme) = store.get(&names[index]) {
            preview(&mut out, &theme)?;
        }

        match terminal.key()? {
            Key::Up => {
                index = if index == 0 {
                    names.len() - 1
                } else {
                    index - 1
                }
            }
            Key::Down => index = (index + 1) % names.len(),
            Key::Enter => {
                write!(out, "\r\n")?;
                return Ok(Choice::Selected(names[index].clone()));
            }
            Key::Cancel => {
                preview(&mut out, current)?;
                write!(out, "\r\n")?;
                return Ok(Choice::Cancelled);
            }
            Key::Other => {}
        }
    }
}

/// Paint the session with a scheme using the standard palette sequences.
fn preview(out: &mut impl Write, theme: &TerminalTheme) -> std::io::Result<()> {
    let colors = [
        &theme.black,
        &theme.red,
        &theme.green,
        &theme.yellow,
        &theme.blue,
        &theme.purple,
        &theme.cyan,
        &theme.white,
        &theme.bright_black,
        &theme.bright_red,
        &theme.bright_green,
        &theme.bright_yellow,
        &theme.bright_blue,
        &theme.bright_purple,
        &theme.bright_cyan,
        &theme.bright_white,
    ];
    for (index, color) in colors.iter().enumerate() {
        write!(out, "\x1b]4;{index};{}\x07", normalize(color))?;
    }
    write!(out, "\x1b]10;{}\x07", normalize(&theme.foreground))?;
    write!(out, "\x1b]11;{}\x07", normalize(&theme.background))?;
    out.flush()
}

/// `#rrggbb` is what terminals accept; configuration allows the `#` to be
/// omitted.
fn normalize(color: &str) -> String {
    let value = color.trim();
    if value.starts_with('#') {
        value.to_string()
    } else {
        format!("#{value}")
    }
}

enum Key {
    Up,
    Down,
    Enter,
    Cancel,
    Other,
}

/// Raw mode for the duration of the picker, restored on drop so a panic or an
/// early return cannot leave the shell without echo.
struct RawTerminal {
    #[cfg(unix)]
    saved: libc::termios,
}

#[cfg(unix)]
impl RawTerminal {
    fn enter() -> std::io::Result<Self> {
        unsafe {
            let mut saved: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut saved) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            let mut raw = saved;
            raw.c_lflag &= !(libc::ICANON | libc::ECHO);
            raw.c_cc[libc::VMIN] = 1;
            raw.c_cc[libc::VTIME] = 0;
            if libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            // Hide the cursor while a list is being redrawn under it.
            let mut out = std::io::stdout();
            let _ = write!(out, "\x1b[?25l");
            let _ = out.flush();
            Ok(Self { saved })
        }
    }

    fn key(&mut self) -> std::io::Result<Key> {
        use std::io::Read;

        let mut byte = [0u8; 1];
        let mut input = std::io::stdin();
        if input.read(&mut byte)? == 0 {
            return Ok(Key::Cancel);
        }
        match byte[0] {
            b'\r' | b'\n' => Ok(Key::Enter),
            b'q' | 0x03 => Ok(Key::Cancel),
            b'k' => Ok(Key::Up),
            b'j' => Ok(Key::Down),
            0x1b => {
                // An escape sequence, or a bare Escape. Read with a timeout so
                // pressing Escape does not wait for a key that never comes.
                let arrow = self.timed_read(2)?;
                match arrow.as_slice() {
                    [b'[', b'A'] => Ok(Key::Up),
                    [b'[', b'B'] => Ok(Key::Down),
                    [] => Ok(Key::Cancel),
                    _ => Ok(Key::Other),
                }
            }
            _ => Ok(Key::Other),
        }
    }

    fn timed_read(&mut self, count: usize) -> std::io::Result<Vec<u8>> {
        unsafe {
            let mut timed = self.saved;
            timed.c_lflag &= !(libc::ICANON | libc::ECHO);
            timed.c_cc[libc::VMIN] = 0;
            // Tenths of a second: long enough for a terminal's own sequence,
            // short enough that Escape feels immediate.
            timed.c_cc[libc::VTIME] = 1;
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &timed);
        }
        use std::io::Read;

        let mut buffer = vec![0u8; count];
        let read = std::io::stdin().read(&mut buffer).unwrap_or(0);
        buffer.truncate(read);
        unsafe {
            let mut raw = self.saved;
            raw.c_lflag &= !(libc::ICANON | libc::ECHO);
            raw.c_cc[libc::VMIN] = 1;
            raw.c_cc[libc::VTIME] = 0;
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw);
        }
        Ok(buffer)
    }
}

#[cfg(unix)]
impl Drop for RawTerminal {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.saved);
        }
        let mut out = std::io::stdout();
        let _ = write!(out, "\x1b[?25h");
        let _ = out.flush();
    }
}

#[cfg(not(unix))]
impl RawTerminal {
    fn enter() -> std::io::Result<Self> {
        Err(std::io::Error::other(
            "interactive picking needs a terminal",
        ))
    }

    fn key(&mut self) -> std::io::Result<Key> {
        Ok(Key::Cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_emits_the_standard_palette_sequences() {
        let mut out = Vec::new();
        let theme = TerminalTheme::default();
        preview(&mut out, &theme).expect("write");
        let text = String::from_utf8(out).expect("utf8");
        assert!(text.contains("\x1b]4;1;#cc6666\x07"), "ANSI red: {text:?}");
        assert!(text.contains("\x1b]10;#ffffff\x07"), "foreground");
        assert!(text.contains("\x1b]11;#282c34\x07"), "background");
        assert_eq!(
            text.matches("\x1b]4;").count(),
            16,
            "the whole palette, so a scheme previews completely"
        );
    }

    #[test]
    fn colors_are_normalized_for_the_terminal() {
        assert_eq!(normalize("#123456"), "#123456");
        assert_eq!(
            normalize("123456"),
            "#123456",
            "the hash is optional in config"
        );
    }
}
