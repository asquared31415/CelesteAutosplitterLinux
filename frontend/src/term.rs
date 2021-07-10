use std::io::{self, Write};

pub fn clear() {
    const CLEAR_CODE: &[u8] = b"\x1B[H\x1B[2J\x1B[3J";

    // Avoid the fmt calls to just print the string
    let stdout = io::stdout();
    stdout
        .lock()
        .write_all(CLEAR_CODE)
        .expect("Unable to clear terminal");
}

pub fn reset_term_style() {
    const RESET_CODE: &[u8] = b"\x1B[0m";

    // Avoid the fmt calls to just print the string
    let stdout = io::stdout();
    stdout
        .lock()
        .write_all(RESET_CODE)
        .expect("Unable to clear terminal");
}

pub fn write<S: AsRef<str>>(s: S, text_color: TermColor, bg_color: Option<TermColor>) {
    print!("\x1B[{}m", text_color.as_code());
    if let Some(bg_color) = bg_color {
        print!("\x1B[{}m", bg_color.as_code() + 10);
    }
    print!("{}", s.as_ref());
    reset_term_style();
}

#[allow(dead_code)]
pub enum TermColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Gray,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl TermColor {
    pub fn as_code(&self) -> u8 {
        match self {
            TermColor::Black => 30,
            TermColor::Red => 31,
            TermColor::Green => 32,
            TermColor::Yellow => 33,
            TermColor::Blue => 34,
            TermColor::Magenta => 35,
            TermColor::Cyan => 36,
            TermColor::White => 37,
            TermColor::Gray => 90,
            TermColor::BrightRed => 91,
            TermColor::BrightGreen => 92,
            TermColor::BrightYellow => 93,
            TermColor::BrightBlue => 94,
            TermColor::BrightMagenta => 95,
            TermColor::BrightCyan => 96,
            TermColor::BrightWhite => 97,
        }
    }
}
