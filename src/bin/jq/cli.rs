use std::{fs::File, io::Read, str::FromStr};

use crate::error::AppError;
use crate::render;
use rs_jq::jql::{jql_execute_lazy_with, jql_execute_opt};
use rs_jq::json::{json_load_slice, value::JsonValue, SIMDParser};
use structopt::StructOpt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum InputFormat {
    JSON,
    AUTO,
}

impl FromStr for InputFormat {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(InputFormat::JSON),
            "auto" => Ok(InputFormat::AUTO),
            other => Err(AppError::InvalidFormat(format!(
                "invalid format \"{other}\", expected json or auto"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseMode {
    Eager,
    Lazy,
}

impl FromStr for ParseMode {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "eager" => Ok(ParseMode::Eager),
            "lazy" => Ok(ParseMode::Lazy),
            other => Err(AppError::InvalidFormat(format!(
                "invalid parse mode \"{other}\", expected eager or lazy"
            ))),
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "rjq", about = "CLI for rs-jq")]
struct Opts {
    #[structopt(short, long, default_value = "")]
    input: String,

    #[structopt(short, long, default_value = "auto")]
    format: InputFormat,

    #[structopt(short = "s", long, default_value = ".")]
    script: String,

    /// Disable colored output (also respects `NO_COLOR`).
    #[structopt(long = "no-color")]
    no_color: bool,

    /// Run AST peephole optimization before executing the query (default: off).
    #[structopt(long = "optimize")]
    optimize: bool,

    /// JSON parse strategy: `eager` (default, full DOM) or `lazy` (path-guided).
    #[structopt(long = "parse", default_value = "eager")]
    parse: ParseMode,

    /// Use the SIMD JSON parser (simdjson-style structural index) instead of
    /// the scalar recursive-descent parser.
    #[structopt(long = "simd")]
    simd: bool,
}

impl Opts {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.input.is_empty() && self.format == InputFormat::AUTO {
            return Err(AppError::InvalidFormat(
                "input is required when format is auto".to_string(),
            ));
        }
        Ok(())
    }
}

fn load_input(input: &str) -> Result<Box<dyn Read>, AppError> {
    match input {
        "" => Ok(Box::new(std::io::stdin())),
        path => Ok(Box::new(File::open(path).map_err(|source| {
            AppError::Io {
                path: Some(path.to_string()),
                source,
            }
        })?)),
    }
}

fn process(mut reader: Box<dyn Read>, opts: &Opts) -> Result<JsonValue<'static>, AppError> {
    match opts.parse {
        ParseMode::Lazy => {
            let mut buf = Vec::new();
            reader
                .read_to_end(&mut buf)
                .map_err(|source| AppError::Io {
                    path: None,
                    source,
                })?;
            Ok(jql_execute_lazy_with(
                &buf,
                &opts.script,
                opts.optimize,
                opts.simd,
            )?)
        }
        ParseMode::Eager => {
            // Keep the input buffer alive so parse can borrow strings (zero-copy).
            // Only the query result is cloned to `'static`.
            let mut buf = Vec::new();
            reader
                .read_to_end(&mut buf)
                .map_err(|source| AppError::Io {
                    path: None,
                    source,
                })?;
            let json_value = if opts.simd {
                SIMDParser::parse_slice(&buf)?
            } else {
                json_load_slice(&buf)?
            };
            let result = jql_execute_opt(&json_value, &opts.script, opts.optimize)?;
            Ok(result.into_owned())
        }
    }
}

pub fn execute() -> Result<(), AppError> {
    let opts = Opts::from_args();

    opts.validate()?;

    let reader = load_input(&opts.input)?;
    let value = process(reader, &opts)?;
    render::print_value(&value, opts.no_color)
        .map_err(|source| AppError::Io { path: None, source })?;

    Ok(())
}
