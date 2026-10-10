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
pub(crate) mod lexer;
pub(crate) mod parser;
pub(crate) mod proxy;
pub(crate) mod reference;
pub(crate) mod token;

pub fn jql_parse<'a>(source: &'a str) -> Result<JqlAstNode<'a>, JqlError> {
    let tokens = JqlLexer::tokenize(source)?;
    JqlParser::parse(&tokens, source)
}

pub fn jql_execute<'a>(value: &'a JsonValue<'a>, query: &str) -> Result<JsonValue<'a>, JqlError> {
    let proxy = Proxy::new(value);
    let ast = jql_parse(query)?;
    let result = Engine::execute(proxy, &ast)?;
    Ok(result.data().clone())
}

#[cfg(test)]
#[path = "../../tests/jql/fixture.rs"]
pub(crate) mod fixture;

#[cfg(test)]
#[path = "../../tests/jql/e2e.rs"]
mod tests;
