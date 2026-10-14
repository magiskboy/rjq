use crate::{
    jql::{
        ast::JqlAstNode, engine::Engine, error::JqlError, lexer::JqlLexer, parser::JqlParser,
        proxy::Proxy,
    },
    json::value::JsonValue,
};

pub(crate) mod ast;
pub(crate) mod engine;
pub mod error;
pub(crate) mod funcs;
pub(crate) mod lexer;
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

#[cfg(test)]
#[path = "../../tests/jql/fixture.rs"]
pub(crate) mod fixture;

#[cfg(test)]
#[path = "../../tests/jql/access.rs"]
mod access_tests;

#[cfg(test)]
#[path = "../../tests/jql/e2e.rs"]
mod tests;
