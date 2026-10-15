use crate::{
    jql::{
        ast::JqlAstNode, engine::Engine, error::JqlError, lexer::JqlLexer, need::required_input,
        parser::JqlParser, proxy::Proxy,
    },
    json::{
        builder::lazy::LazyJsonValueBuilder, parser::JsonParser, value::JsonValue, SIMDParser,
    },
};

pub(crate) mod ast;
pub(crate) mod engine;
pub mod error;
pub(crate) mod funcs;
pub(crate) mod lexer;
pub(crate) mod need;
pub(crate) mod optimizer;
pub(crate) mod parser;
pub(crate) mod proxy;
pub(crate) mod token;

/// Input to [`jql_execute`]: an already-built DOM, or raw JSON bytes to parse.
#[derive(Debug, Clone, Copy)]
pub enum JqlInput<'a> {
    Value(&'a JsonValue<'a>),
    Bytes(&'a [u8]),
}

impl<'a> From<&'a JsonValue<'a>> for JqlInput<'a> {
    fn from(value: &'a JsonValue<'a>) -> Self {
        Self::Value(value)
    }
}

impl<'a> From<&'a [u8]> for JqlInput<'a> {
    fn from(data: &'a [u8]) -> Self {
        Self::Bytes(data)
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for JqlInput<'a> {
    fn from(data: &'a [u8; N]) -> Self {
        Self::Bytes(data.as_slice())
    }
}

/// Flags controlling parse/optimize behaviour for [`jql_execute`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JqlExecuteArgs {
    pub optimize: bool,
    /// Selective parse via [`LazyJsonValueBuilder`] (only for [`JqlInput::Bytes`]).
    pub lazy: bool,
    /// Use [`SIMDParser`] when parsing bytes (only for [`JqlInput::Bytes`]).
    pub simd: bool,
}

pub fn jql_parse<'a>(source: &'a str) -> Result<JqlAstNode<'a>, JqlError> {
    let tokens = JqlLexer::tokenize(source)?;
    JqlParser::parse(&tokens, source)
}

pub fn jql_execute<'a>(
    input: impl Into<JqlInput<'a>>,
    query: &str,
    args: JqlExecuteArgs,
) -> Result<JsonValue<'static>, JqlError> {
    let mut ast = jql_parse(query)?;
    if args.optimize {
        ast = Engine::optimize(ast);
    }

    match input.into() {
        JqlInput::Value(value) => {
            let result = Engine::execute(Proxy::new(value), &ast)?;
            Ok(result.into_value().into_owned())
        }
        JqlInput::Bytes(data) => {
            let value = if args.lazy {
                let need = required_input(&ast);
                let mut hook = LazyJsonValueBuilder::new(need);
                if args.simd {
                    SIMDParser::parse_slice_with(data, &mut hook)?
                } else {
                    JsonParser::parse_slice_with(data, &mut hook)?
                }
            } else if args.simd {
                SIMDParser::parse_slice(data)?
            } else {
                JsonParser::parse_slice(data)?
            };
            let result = Engine::execute(Proxy::new(&value), &ast)?;
            Ok(result.into_value().into_owned())
        }
    }
}

#[cfg(test)]
#[path = "../../tests/jql/fixture.rs"]
pub(crate) mod fixture;

#[cfg(test)]
#[path = "../../tests/jql/access.rs"]
mod access_tests;

#[cfg(test)]
#[path = "../../tests/jql/e2e.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/jql/lazy.rs"]
mod lazy_tests;
