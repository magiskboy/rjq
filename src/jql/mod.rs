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

pub fn jql_parse<'a>(source: &'a str) -> Result<JqlAstNode<'a>, JqlError> {
    let tokens = JqlLexer::tokenize(source)?;
    JqlParser::parse(&tokens, source)
}

pub fn jql_execute<'a>(value: &'a JsonValue<'a>, query: &str) -> Result<JsonValue<'a>, JqlError> {
    jql_execute_opt(value, query, false)
}

/// Execute `query` against `value`. When `optimize` is true, run
/// [`Engine::optimize`] on the parsed AST before evaluation.
pub fn jql_execute_opt<'a>(
    value: &'a JsonValue<'a>,
    query: &str,
    optimize: bool,
) -> Result<JsonValue<'a>, JqlError> {
    let proxy = Proxy::new(value);
    let mut ast = jql_parse(query)?;
    if optimize {
        ast = Engine::optimize(ast);
    }
    let result = Engine::execute(proxy, &ast)?;
    Ok(result.into_value())
}

/// Parse `data` with [`LazyJsonValueBuilder`] guided by `query`, then run the
/// **unchanged** engine. Expression is analyzed before JSON bytes are read.
pub fn jql_execute_lazy(
    data: &[u8],
    query: &str,
    optimize: bool,
) -> Result<JsonValue<'static>, JqlError> {
    jql_execute_lazy_with(data, query, optimize, false)
}

/// Like [`jql_execute_lazy`], but optionally uses [`SIMDParser`] for stage-1/2 JSON.
pub fn jql_execute_lazy_with(
    data: &[u8],
    query: &str,
    optimize: bool,
    simd: bool,
) -> Result<JsonValue<'static>, JqlError> {
    let mut ast = jql_parse(query)?;
    if optimize {
        ast = Engine::optimize(ast);
    }
    let need = required_input(&ast);
    let mut hook = LazyJsonValueBuilder::new(need);
    let value = if simd {
        SIMDParser::parse_slice_with(data, &mut hook)?
    } else {
        JsonParser::parse_slice_with(data, &mut hook)?
    };
    let result = Engine::execute(Proxy::new(&value), &ast)?;
    Ok(result.into_value().into_owned())
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
