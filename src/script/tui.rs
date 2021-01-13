use gluon::{vm::ExternModule, Thread};

#[derive(Clone, Debug, VmType, Pushable, Getable)]
enum Color {
    Reset,
    Rgb(u8, u8, u8),
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    LightBlack,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    LightWhite,
}

use termion::{color, style};

fn fg(c: Color) -> String {
    match c {
        Color::Reset => color::Reset.fg_str().to_string(),
        Color::Rgb(r, g, b) => color::Rgb(r, g, b).fg_string(),
        Color::Black => color::Black.fg_str().to_string(),
        Color::Red => color::Red.fg_str().to_string(),
        Color::Green => color::Green.fg_str().to_string(),
        Color::Yellow => color::Yellow.fg_str().to_string(),
        Color::Blue => color::Blue.fg_str().to_string(),
        Color::Magenta => color::Magenta.fg_str().to_string(),
        Color::Cyan => color::Cyan.fg_str().to_string(),
        Color::White => color::White.fg_str().to_string(),
        Color::LightBlack => color::LightBlack.fg_str().to_string(),
        Color::LightRed => color::LightRed.fg_str().to_string(),
        Color::LightGreen => color::LightGreen.fg_str().to_string(),
        Color::LightYellow => color::LightYellow.fg_str().to_string(),
        Color::LightBlue => color::LightBlue.fg_str().to_string(),
        Color::LightMagenta => color::LightMagenta.fg_str().to_string(),
        Color::LightCyan => color::LightCyan.fg_str().to_string(),
        Color::LightWhite => color::LightWhite.fg_str().to_string(),
    }
}

fn bg(c: Color) -> String {
    match c {
        Color::Reset => color::Reset.bg_str().to_string(),
        Color::Rgb(r, g, b) => color::Rgb(r, g, b).bg_string(),
        Color::Black => color::Black.bg_str().to_string(),
        Color::Red => color::Red.bg_str().to_string(),
        Color::Green => color::Green.bg_str().to_string(),
        Color::Yellow => color::Yellow.bg_str().to_string(),
        Color::Blue => color::Blue.bg_str().to_string(),
        Color::Magenta => color::Magenta.bg_str().to_string(),
        Color::Cyan => color::Cyan.bg_str().to_string(),
        Color::White => color::White.bg_str().to_string(),
        Color::LightBlack => color::LightBlack.bg_str().to_string(),
        Color::LightRed => color::LightRed.bg_str().to_string(),
        Color::LightGreen => color::LightGreen.bg_str().to_string(),
        Color::LightYellow => color::LightYellow.bg_str().to_string(),
        Color::LightBlue => color::LightBlue.bg_str().to_string(),
        Color::LightMagenta => color::LightMagenta.bg_str().to_string(),
        Color::LightCyan => color::LightCyan.bg_str().to_string(),
        Color::LightWhite => color::LightWhite.bg_str().to_string(),
    }
}

pub fn load(thread: &Thread) -> Result<ExternModule, gluon::vm::Error> {
    ExternModule::new(
        thread,
        record! {
            fg => primitive!(1, fg),
            bg => primitive!(1, bg),
            rgb => primitive!(3, Color::Rgb),
            no_color => Color::Reset,
            black => Color::Black,
            red => Color::Red,
            green => Color::Green,
            yellow => Color::Yellow,
            blue => Color::Blue,
            magenta => Color::Magenta,
            cyan => Color::Cyan,
            white => Color::White,
            light_black => Color::LightBlack,
            light_red => Color::LightRed,
            light_green => Color::LightGreen,
            light_yellow => Color::LightYellow,
            light_blue => Color::LightBlue,
            light_magenta => Color::LightMagenta,
            light_cyan => Color::LightCyan,
            light_white => Color::LightWhite,
            no_style => style::Reset.to_string(),
            bold => style::Bold.to_string(),
            italic => style::Italic.to_string(),
            underline => style::Underline.to_string(),
            no_bold => style::NoBold.to_string(),
            no_italic => style::NoItalic.to_string(),
            no_underline => style::NoUnderline.to_string(),
        },
    )
}
