use std::{fs::File, io::Read, str::FromStr};

use crate::error::AppError;
use crate::render;
use rs_jq::jql::jql_execute_opt;
use rs_jq::json::{json_load, value::JsonValue};
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

fn process(reader: Box<dyn Read>, opts: &Opts) -> Result<JsonValue<'static>, AppError> {
    let json_value = json_load(reader)?;
    let result = jql_execute_opt(&json_value, &opts.script, opts.optimize)?;
    Ok(result.into_owned())
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
