use std::io::{self, Write};

use crate::color::{ColorLevel, ColorScheme, Rgb};
use rs_jq::json::value::JsonValue;

#[derive(Debug, Clone)]
struct Style {
    open: String,
}

impl Style {
    fn plain() -> Self {
        Self {
            open: String::new(),
        }
    }

    fn from_rgb(rgb: Rgb, level: ColorLevel) -> Self {
        Self {
            open: rgb.ansi_open(level),
        }
    }
}

#[derive(Debug, Clone)]
struct Theme {
    key: Style,
    string: Style,
    number: Style,
    boolean: Style,
    null: Style,
    punct: Style,
}

impl Theme {
    fn from_scheme(scheme: &ColorScheme, level: ColorLevel) -> Self {
        if level == ColorLevel::None {
            return Self::plain();
        }
        Self {
            key: Style::from_rgb(scheme.key, level),
            string: Style::from_rgb(scheme.string, level),
            number: Style::from_rgb(scheme.number, level),
            boolean: Style::from_rgb(scheme.boolean, level),
            null: Style::from_rgb(scheme.null, level),
            punct: Style::from_rgb(scheme.punct, level),
        }
    }

    fn plain() -> Self {
        Self {
            key: Style::plain(),
            string: Style::plain(),
            number: Style::plain(),
            boolean: Style::plain(),
            null: Style::plain(),
            punct: Style::plain(),
        }
    }
}

/// Pretty-print `value` to stdout with syntax highlighting when supported.
///
/// When `no_color` is true, ANSI colors are disabled regardless of the
/// environment. Otherwise the scheme is loaded from [`crate::color::COLORS_ENV`]
/// (or the default) and adapted to the detected [`ColorLevel`].
pub fn print_value(value: &JsonValue<'_>, no_color: bool) -> io::Result<()> {
    let mut stdout = io::stdout().lock();

    if no_color {
        write!(stdout, "{}", value)?;
        return Ok(());
    }

    let level = ColorLevel::detect();
    let scheme = ColorScheme::from_env();
    render_value(&mut stdout, value, level, &scheme)?;
    writeln!(stdout)?;
    Ok(())
}

/// Pretty-print `value` into `writer` using the given color level and scheme.
pub fn render_value(
    writer: &mut impl Write,
    value: &JsonValue<'_>,
    level: ColorLevel,
    scheme: &ColorScheme,
) -> io::Result<()> {
    let theme = Theme::from_scheme(scheme, level);
    dump(writer, value, &theme, 0)
}

fn paint(writer: &mut impl Write, style: &Style, text: &str) -> io::Result<()> {
    if style.open.is_empty() {
        writer.write_all(text.as_bytes())
    } else {
        write!(writer, "{}{}\x1b[0m", style.open, text)
    }
}

fn write_indent(writer: &mut impl Write, depth: usize) -> io::Result<()> {
    for _ in 0..depth {
        writer.write_all(b"  ")?;
    }
    Ok(())
}

fn dump(
    writer: &mut impl Write,
    value: &JsonValue<'_>,
    theme: &Theme,
    depth: usize,
) -> io::Result<()> {
    match value {
        JsonValue::Null => paint(writer, &theme.null, "null"),
        JsonValue::True => paint(writer, &theme.boolean, "true"),
        JsonValue::False => paint(writer, &theme.boolean, "false"),
        JsonValue::Number(n) => paint(writer, &theme.number, n.raw()),
        JsonValue::String(s) => write_colored_string(writer, &theme.string, s.as_ref()),
        JsonValue::Array(items) => {
            paint(writer, &theme.punct, "[")?;
            if !items.is_empty() {
                writeln!(writer)?;
                for (i, item) in items.iter().enumerate() {
                    write_indent(writer, depth + 1)?;
                    dump(writer, item, theme, depth + 1)?;
                    if i + 1 < items.len() {
                        paint(writer, &theme.punct, ",")?;
                    }
                    writeln!(writer)?;
                }
                write_indent(writer, depth)?;
            }
            paint(writer, &theme.punct, "]")
        }
        JsonValue::Object(object) => {
            paint(writer, &theme.punct, "{")?;
            if !object.is_empty() {
                writeln!(writer)?;
                for (i, (key, child)) in object.iter().enumerate() {
                    write_indent(writer, depth + 1)?;
                    write_colored_string(writer, &theme.key, key.as_ref())?;
                    paint(writer, &theme.punct, ": ")?;
                    dump(writer, child, theme, depth + 1)?;
                    if i + 1 < object.len() {
                        paint(writer, &theme.punct, ",")?;
                    }
                    writeln!(writer)?;
                }
                write_indent(writer, depth)?;
            }
            paint(writer, &theme.punct, "}")
        }
    }
}

fn write_colored_string(writer: &mut impl Write, style: &Style, value: &str) -> io::Result<()> {
    if !style.open.is_empty() {
        writer.write_all(style.open.as_bytes())?;
    }
    writer.write_all(b"\"")?;
    write_escaped(writer, value)?;
    writer.write_all(b"\"")?;
    if !style.open.is_empty() {
        writer.write_all(b"\x1b[0m")?;
    }
    Ok(())
}

fn write_escaped(writer: &mut impl Write, value: &str) -> io::Result<()> {
    for ch in value.chars() {
        match ch {
            '"' => writer.write_all(br#"\""#)?,
            '\\' => writer.write_all(br"\\")?,
            '\n' => writer.write_all(br"\n")?,
            '\r' => writer.write_all(br"\r")?,
            '\t' => writer.write_all(br"\t")?,
            '\u{08}' => writer.write_all(br"\b")?,
            '\u{0C}' => writer.write_all(br"\f")?,
            ch if ch.is_control() => write!(writer, "\\u{:04x}", ch as u32)?,
            ch => {
                let mut buf = [0; 4];
                writer.write_all(ch.encode_utf8(&mut buf).as_bytes())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs_jq::json::value::JsonValue;

    #[test]
    fn render_plain_matches_structure() {
        let value = JsonValue::object([
            ("name", JsonValue::string("Ada")),
            ("n", JsonValue::number(1.0)),
            ("ok", JsonValue::True),
            ("missing", JsonValue::Null),
        ]);
        let mut buf = Vec::new();
        render_value(&mut buf, &value, ColorLevel::None, &ColorScheme::DEFAULT).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("\"name\": \"Ada\""));
        assert!(text.contains("\"n\": 1"));
        assert!(text.contains("\"ok\": true"));
        assert!(text.contains("\"missing\": null"));
        assert!(!text.contains('\x1b'));
    }

    #[test]
    fn render_truecolor_uses_scheme_rgb() {
        let scheme = ColorScheme {
            string: Rgb::new(1, 2, 3),
            ..ColorScheme::DEFAULT
        };
        let value = JsonValue::string("hi");
        let mut buf = Vec::new();
        render_value(&mut buf, &value, ColorLevel::TrueColor, &scheme).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("\x1b[38;2;1;2;3m"));
        assert!(text.contains("\"hi\""));
        assert!(text.contains("\x1b[0m"));
    }

    #[test]
    fn render_basic_emits_ansi() {
        let value = JsonValue::string("hi");
        let mut buf = Vec::new();
        render_value(&mut buf, &value, ColorLevel::Basic, &ColorScheme::DEFAULT).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("\x1b["));
        assert!(text.contains("\"hi\""));
        assert!(text.contains("\x1b[0m"));
    }
}
