use std::collections::VecDeque;

use crate::Span;
use crate::jql::ast::JqlNumericKind;
use crate::jql::{
    ast::{JqlAstNode, JqlBinaryKind, ObjectField},
    error::{ExpectedSyntax, JqlError, JqlErrorKind},
    token::{JqlToken, JqlTokenKind},
};

#[derive(Debug, Clone)]
pub struct JqlParser;

impl JqlParser {
    fn unexpected_token(
        expected: ExpectedSyntax,
        found: JqlTokenKind,
        span: Span,
        source: &str,
    ) -> JqlError {
        JqlError::new(
            JqlErrorKind::UnexpectedToken { expected, found },
            span,
            source,
        )
    }

    fn missing_operand(span: Span, source: &str) -> JqlError {
        JqlError::new(JqlErrorKind::MissingOperand, span, source)
    }

    fn apply_operator<'a>(
        op: JqlTokenKind,
        left: JqlAstNode<'a>,
        right: JqlAstNode<'a>,
    ) -> Result<JqlAstNode<'a>, JqlError> {
        match op {
            JqlTokenKind::Pipe => JqlAstNode::pipe(left, right),
            JqlTokenKind::Plus
            | JqlTokenKind::Subtract
            | JqlTokenKind::Multiply
            | JqlTokenKind::Divide
            | JqlTokenKind::Mod => {
                JqlAstNode::numeric(JqlNumericKind::from_token_kind(&op)?, left, right)
            }
            JqlTokenKind::LessOp
            | JqlTokenKind::GreaterOp
            | JqlTokenKind::LessEqualOp
            | JqlTokenKind::GreaterEqualOp
            | JqlTokenKind::EqualOp
            | JqlTokenKind::NotEqualOp
            | JqlTokenKind::AndLogicalOp
            | JqlTokenKind::OrLogicalOp => {
                JqlAstNode::binary(JqlBinaryKind::from_token_kind(&op)?, left, right)
            }
            _ => unreachable!(),
        }
    }

    /// Scan forward from `start` until the matching closer for an opener already counted
    /// in `depth` (typically 1). Returns the index of the closer, or `None` if unclosed.
    fn find_matching_close(
        tokens: &[JqlToken],
        start: usize,
        open: JqlTokenKind,
        close: JqlTokenKind,
        mut depth: i32,
    ) -> Option<usize> {
        let mut end = start;
        while end < tokens.len() {
            match tokens[end].kind {
                kind if kind == open => depth += 1,
                kind if kind == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(end);
                    }
                }
                _ => {}
            }
            end += 1;
        }
        None
    }

    fn parse_object<'a>(tokens: &[JqlToken], source: &'a str) -> Result<JqlAstNode<'a>, JqlError> {
        if tokens.is_empty() {
            return JqlAstNode::object(vec![]);
        }

        let mut fields = Vec::new();
        let mut idx: usize = 0;

        while idx < tokens.len() {
            let key_token = tokens[idx];
            let key = JqlAstNode::object_key(&key_token, source)?;
            idx += 1;

            let colon = tokens.get(idx).ok_or_else(|| {
                Self::unexpected_token(
                    ExpectedSyntax::Token(JqlTokenKind::Colon),
                    JqlTokenKind::Stop,
                    key_token.span,
                    source,
                )
            })?;
            if colon.kind != JqlTokenKind::Colon {
                return Err(Self::unexpected_token(
                    ExpectedSyntax::Token(JqlTokenKind::Colon),
                    colon.kind,
                    colon.span,
                    source,
                ));
            }
            idx += 1;

            let value_start = idx;
            let mut depth_paren = 0_i32;
            let mut depth_brace = 0_i32;
            while idx < tokens.len() {
                match tokens[idx].kind {
                    JqlTokenKind::LParen => depth_paren += 1,
                    JqlTokenKind::RParen => depth_paren -= 1,
                    JqlTokenKind::LBrace => depth_brace += 1,
                    JqlTokenKind::RBrace => depth_brace -= 1,
                    JqlTokenKind::Comma if depth_paren == 0 && depth_brace == 0 => break,
                    _ => {}
                }
                idx += 1;
            }

            if value_start == idx {
                let span = tokens.get(idx).map(|t| t.span).unwrap_or(key_token.span);
                return Err(JqlError::new(JqlErrorKind::InvalidExpression, span, source));
            }

            let value = Self::parse(&tokens[value_start..idx], source)?;
            fields.push(ObjectField {
                key,
                value: Box::new(value),
            });

            if idx < tokens.len() {
                if tokens[idx].kind != JqlTokenKind::Comma {
                    return Err(Self::unexpected_token(
                        ExpectedSyntax::Token(JqlTokenKind::Comma),
                        tokens[idx].kind,
                        tokens[idx].span,
                        source,
                    ));
                }
                idx += 1;
                if idx >= tokens.len() {
                    return Err(JqlError::new(
                        JqlErrorKind::InvalidExpression,
                        tokens[idx - 1].span,
                        source,
                    ));
                }
            }
        }

        JqlAstNode::object(fields)
    }

    fn parse_call<'a>(
        tokens: &[JqlToken],
        idx: usize,
        token: JqlToken,
        source: &'a str,
    ) -> Result<(JqlAstNode<'a>, usize), JqlError> {
        let Some(next_token) = tokens.get(idx + 1) else {
            return Ok((JqlAstNode::call(&token, vec![], source)?, idx + 1));
        };

        if next_token.kind != JqlTokenKind::LParen {
            return Ok((JqlAstNode::call(&token, vec![], source)?, idx + 1));
        }

        let Some(end) = Self::find_matching_close(
            tokens,
            idx + 2,
            JqlTokenKind::LParen,
            JqlTokenKind::RParen,
            1,
        ) else {
            return Err(JqlError::new(
                JqlErrorKind::UnclosedParen,
                token.span,
                source,
            ));
        };

        let arg = Self::parse(&tokens[idx + 2..end], source)?;
        let node = JqlAstNode::call(&token, vec![arg], source)?;
        Ok((node, end + 1))
    }

    pub fn parse<'a>(tokens: &[JqlToken], source: &'a str) -> Result<JqlAstNode<'a>, JqlError> {
        let mut operator_stack = VecDeque::<JqlToken>::new();
        let mut operand_stack = VecDeque::<JqlAstNode>::new();
        let mut idx: usize = 0;

        while idx < tokens.len() {
            let token = tokens[idx];

            match token.kind {
                JqlTokenKind::LBrace => {
                    let Some(end) = Self::find_matching_close(
                        tokens,
                        idx + 1,
                        JqlTokenKind::LBrace,
                        JqlTokenKind::RBrace,
                        1,
                    ) else {
                        return Err(JqlError::new(
                            JqlErrorKind::UnclosedBrace,
                            token.span,
                            source,
                        ));
                    };
                    let inner = &tokens[idx + 1..end];
                    operand_stack.push_front(Self::parse_object(inner, source)?);
                    idx = end + 1;
                }

                JqlTokenKind::LParen => {
                    idx += 1;
                    operator_stack.push_front(token);
                }

                JqlTokenKind::RParen => {
                    idx += 1;
                    while let Some(op) = operator_stack.pop_front() {
                        if op.kind == JqlTokenKind::LParen {
                            break;
                        }

                        let right = operand_stack
                            .pop_front()
                            .ok_or_else(|| Self::missing_operand(token.span, source))?;
                        let left = operand_stack
                            .pop_front()
                            .ok_or_else(|| Self::missing_operand(token.span, source))?;
                        let node = Self::apply_operator(op.kind, left, right)?;
                        operand_stack.push_front(node);
                    }
                }

                JqlTokenKind::Null => {
                    idx += 1;
                    operand_stack.push_front(JqlAstNode::null(&token)?);
                }
                JqlTokenKind::String => {
                    idx += 1;
                    operand_stack.push_front(JqlAstNode::string(&token, source)?);
                }
                JqlTokenKind::Boolean => {
                    idx += 1;
                    operand_stack.push_front(JqlAstNode::bool(&token, source)?);
                }
                JqlTokenKind::Number => {
                    idx += 1;
                    operand_stack.push_front(JqlAstNode::number(&token, source)?);
                }

                JqlTokenKind::Identifier => {
                    let token = tokens.get(idx).unwrap();
                    let content = source.get(token.span.start..token.span.end).unwrap();
                    if content.starts_with('.') {
                        operand_stack.push_front(JqlAstNode::identifier(token, source)?);
                        idx += 1;
                    } else {
                        let (node, next_idx) = Self::parse_call(tokens, idx, *token, source)?;
                        operand_stack.push_front(node);
                        idx = next_idx;
                    }
                }

                JqlTokenKind::Pipe
                | JqlTokenKind::Plus
                | JqlTokenKind::Subtract
                | JqlTokenKind::Multiply
                | JqlTokenKind::Divide
                | JqlTokenKind::Mod
                | JqlTokenKind::LessOp
                | JqlTokenKind::LessEqualOp
                | JqlTokenKind::GreaterOp
                | JqlTokenKind::GreaterEqualOp
                | JqlTokenKind::EqualOp
                | JqlTokenKind::NotEqualOp
                | JqlTokenKind::AndLogicalOp
                | JqlTokenKind::OrLogicalOp => {
                    while let Some(top) = operator_stack.pop_front() {
                        if top.kind > token.kind {
                            let right = operand_stack
                                .pop_front()
                                .ok_or_else(|| Self::missing_operand(token.span, source))?;
                            let left = operand_stack
                                .pop_front()
                                .ok_or_else(|| Self::missing_operand(token.span, source))?;
                            let node = Self::apply_operator(top.kind, left, right)?;
                            operand_stack.push_front(node);
                        } else {
                            operator_stack.push_front(top);
                            break;
                        }
                    }
                    idx += 1;
                    operator_stack.push_front(token);
                }

                JqlTokenKind::Whitespace | JqlTokenKind::Stop => {
                    idx += 1;
                }

                other => {
                    return Err(Self::unexpected_token(
                        ExpectedSyntax::Expression,
                        other,
                        token.span,
                        source,
                    ));
                }
            }
        }

        while let Some(top_token) = operator_stack.pop_front() {
            let right = operand_stack
                .pop_front()
                .ok_or_else(|| Self::missing_operand(top_token.span, source))?;
            let left = operand_stack
                .pop_front()
                .ok_or_else(|| Self::missing_operand(top_token.span, source))?;
            let node = Self::apply_operator(top_token.kind, left, right)?;
            operand_stack.push_front(node);
        }

        if operand_stack.len() != 1 {
            return Err(JqlError::new(
                JqlErrorKind::InvalidExpression,
                Span {
                    start: 0,
                    end: source.len(),
                },
                source,
            ));
        }

        Ok(operand_stack.pop_front().unwrap())
    }
}

#[cfg(test)]
#[path = "../../tests/jql/parser.rs"]
mod test;
