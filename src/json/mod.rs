use std::io::Read;

use crate::json::{error::JsonError, parser::JsonParser, value::JsonValue};

pub mod error;
pub mod hook;
pub mod lazy;
pub(crate) mod escape;
pub(crate) mod lexer;
pub mod number;
pub(crate) mod parser;
pub(crate) mod scan;
pub(crate) mod source;
pub(crate) mod token;
pub mod value;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct JsonDumpOptions {
    indent: String,
    pretty: bool,
}

impl Default for JsonDumpOptions {
    fn default() -> Self {
        Self {
            indent: "  ".to_string(),
            pretty: true,
        }
    }
}

pub fn json_load(reader: impl Read) -> Result<JsonValue<'static>, JsonError> {
    JsonParser::parse_reader(reader)
}

/// Parse JSON from an in-memory slice with zero-copy string borrows when possible.
pub fn json_load_slice<'a>(data: &'a [u8]) -> Result<JsonValue<'a>, JsonError> {
    JsonParser::parse_slice(data)
}

/// Parse JSON from an in-memory string with zero-copy string borrows when possible.
pub fn json_load_str<'a>(source: &'a str) -> Result<JsonValue<'a>, JsonError> {
    JsonParser::parse_str(source)
}

pub fn json_dump(
    writer: &mut impl std::io::Write,
    value: &JsonValue<'_>,
    opts: &JsonDumpOptions,
) -> Result<(), std::io::Error> {
    dump(value, writer, opts, 0)
}

pub fn json_dumps(
    writer: &mut impl std::fmt::Write,
    value: &JsonValue<'_>,
    opts: &JsonDumpOptions,
) -> std::fmt::Result {
    let mut bytes = Vec::new();
    dump(value, &mut bytes, opts, 0).map_err(|_| std::fmt::Error)?;
    let string = String::from_utf8(bytes).map_err(|_| std::fmt::Error)?;
    write!(writer, "{}", string)
}

fn write_indent(
    writer: &mut impl std::io::Write,
    depth: usize,
    opts: &JsonDumpOptions,
) -> std::io::Result<()> {
    for _ in 0..depth {
        writer.write_all(opts.indent.as_bytes())?;
    }
    Ok(())
}

fn dump(
    value: &JsonValue<'_>,
    writer: &mut impl std::io::Write,
    opts: &JsonDumpOptions,
    depth: usize,
) -> Result<(), std::io::Error> {
    match value {
        JsonValue::Null => write!(writer, "null"),
        JsonValue::True => write!(writer, "true"),
        JsonValue::False => write!(writer, "false"),
        JsonValue::Number(n) => write!(writer, "{}", n.raw()),
        JsonValue::String(st) => write_json_string(writer, st.as_ref()),
        JsonValue::Array(items) => {
            write!(writer, "[")?;
            if !items.is_empty() {
                if opts.pretty {
                    writeln!(writer)?;
                }

                for (i, item) in items.iter().enumerate() {
                    if opts.pretty {
                        write_indent(writer, depth + 1, opts)?;
                    }

                    dump(item, writer, opts, depth + 1)?;

                    if i + 1 < items.len() {
                        write!(writer, ",")?;
                    }

                    if opts.pretty {
                        writeln!(writer)?;
                    }
                }
                if opts.pretty {
                    write_indent(writer, depth, opts)?;
                }
            }
            write!(writer, "]")
        }
        JsonValue::Object(object) => {
            write!(writer, "{{")?;

            if !object.is_empty() {
                writeln!(writer)?;
            }

            for (i, (key, value)) in object.iter().enumerate() {
                if opts.pretty {
                    write_indent(writer, depth + 1, opts)?;

                    write_json_string(writer, key)?;
                    if opts.pretty {
                        write!(writer, ": ")?;
                    } else {
                        write!(writer, ":")?;
                    }

                    dump(value, writer, opts, depth + 1)?;

                    if i + 1 < object.len() {
                        write!(writer, ",")?;
                    }

                    if opts.pretty {
                        writeln!(writer)?;
                    }
                }
            }

            if opts.pretty {
                write_indent(writer, depth, opts)?;
            }

            write!(writer, "}}")
        }
    }
}

fn write_json_string(writer: &mut impl std::io::Write, value: &str) -> std::io::Result<()> {
    write!(writer, "\"")?;

    for ch in value.chars() {
        match ch {
            '"' => writer.write_all(br#"\""#)?,
            '\\' => writer.write_all(br"\")?,
            '\n' => writer.write_all(br"\n")?,
            '\r' => writer.write_all(br"\r")?,
            '\t' => writer.write_all(br"\t")?,
            '\u{08}' => writer.write_all(br"\b")?,
            '\u{0C}' => writer.write_all(br"\f")?,

            ch if ch.is_control() => {
                write!(writer, "\\u{:04x}", ch as u32)?;
            }

            ch => {
                let mut buffer = [0; 4];
                writer.write_all(ch.encode_utf8(&mut buffer).as_bytes())?;
            }
        }
    }

    write!(writer, "\"")
}
