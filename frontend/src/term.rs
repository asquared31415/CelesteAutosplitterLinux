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

pub fn write<S: AsRef<str>, C1: Into<Option<TermColor>>, C2: Into<Option<TermColor>>>(
    s: S,
    text_color: C1,
    bg_color: C2,
) {
    if let Some(fg_color) = text_color.into() {
        let fg_code = match fg_color.into() {
            TermColor::Name(name) => name.as_code().to_string(),
            TermColor::Hex(hex) => hex.as_code(),
        };
        print!("\x1B[{}m", fg_code);
    }

    if let Some(bg_color) = bg_color.into() {
        let bg_code = match bg_color.into() {
            TermColor::Name(name) => (name.as_code() + 10).to_string(),
            TermColor::Hex(hex) => hex.as_code(),
        };
        print!("\x1B[{}m", bg_code);
    }

    print!("{}", s.as_ref());
    reset_term_style();
}

pub fn writeln<S: AsRef<str>, C1: Into<Option<TermColor>>, C2: Into<Option<TermColor>>>(
    s: S,
    text_color: C1,
    bg_color: C2,
) {
    write(s, text_color, bg_color);
    write("\n", ColorName::White, None);
}

pub struct HexColor(u8, u8, u8);

impl HexColor {
    pub fn as_code(&self) -> String {
        format!("38;2;{};{};{}", self.0, self.1, self.2)
    }
}

impl Into<Option<TermColor>> for HexColor {
    fn into(self) -> std::option::Option<TermColor> {
        Some(TermColor::Hex(self))
    }
}

#[allow(dead_code)]
pub enum ColorName {
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

impl ColorName {
    pub fn as_code(&self) -> u8 {
        match self {
            ColorName::Black => 30,
            ColorName::Red => 31,
            ColorName::Green => 32,
            ColorName::Yellow => 33,
            ColorName::Blue => 34,
            ColorName::Magenta => 35,
            ColorName::Cyan => 36,
            ColorName::White => 37,
            ColorName::Gray => 90,
            ColorName::BrightRed => 91,
            ColorName::BrightGreen => 92,
            ColorName::BrightYellow => 93,
            ColorName::BrightBlue => 94,
            ColorName::BrightMagenta => 95,
            ColorName::BrightCyan => 96,
            ColorName::BrightWhite => 97,
        }
    }
}

impl Into<Option<TermColor>> for ColorName {
    fn into(self) -> std::option::Option<TermColor> {
        Some(TermColor::Name(self))
    }
}

#[allow(dead_code)]
pub enum TermColor {
    Name(ColorName),
    Hex(HexColor),
}
