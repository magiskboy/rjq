use crate::jql::{
    ast::{
        JqlAstNode::{self},
        JqlBinaryKind, ObjectField,
    },
    error::{ExpectedSyntax, JqlError, JqlErrorKind},
    token::{
        JqlToken,
        JqlTokenKind::{self},
    },
};
use crate::Span;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct JqlParser;

impl JqlParser {
    fn parse_object<'a>(
        tokens: &[JqlToken],
        source: &'a str,
    ) -> Result<JqlAstNode<'a>, JqlError> {
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
                JqlError::new(
                    JqlErrorKind::UnexpectedToken {
                        expected: ExpectedSyntax::Token(JqlTokenKind::Colon),
                        found: JqlTokenKind::Stop,
                    },
                    key_token.span,
                    source,
                )
            })?;
            if colon.kind != JqlTokenKind::Colon {
                return Err(JqlError::new(
                    JqlErrorKind::UnexpectedToken {
                        expected: ExpectedSyntax::Token(JqlTokenKind::Colon),
                        found: colon.kind,
                    },
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
                let span = tokens
                    .get(idx)
                    .map(|t| t.span)
                    .unwrap_or(key_token.span);
                return Err(JqlError::new(JqlErrorKind::InvalidExpression, span, source));
            }

            let value = Self::parse(&tokens[value_start..idx].to_vec(), source)?;
            fields.push(ObjectField {
                key,
                value: Box::new(value),
            });

            if idx < tokens.len() {
                if tokens[idx].kind != JqlTokenKind::Comma {
                    return Err(JqlError::new(
                        JqlErrorKind::UnexpectedToken {
                            expected: ExpectedSyntax::Token(JqlTokenKind::Comma),
                            found: tokens[idx].kind,
                        },
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

    pub fn parse<'a>(tokens: &Vec<JqlToken>, source: &'a str) -> Result<JqlAstNode<'a>, JqlError> {
        let mut operator_stack = VecDeque::<JqlToken>::new();
        let mut operand_stack = VecDeque::<JqlAstNode>::new();

        let mut idx: usize = 0;

        while idx < tokens.len() {
            let token = tokens[idx];

            match token.kind {
                JqlTokenKind::LBrace => {
                    let mut depth = 1;
                    let mut end = idx + 1;
                    while end < tokens.len() {
                        match tokens[end].kind {
                            JqlTokenKind::LBrace => depth += 1,
                            JqlTokenKind::RBrace => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        end += 1;
                    }
                    if depth != 0 {
                        return Err(JqlError::new(
                            JqlErrorKind::UnclosedBrace,
                            token.span,
                            source,
                        ));
                    }
                    let inner = tokens[idx + 1..end].to_vec();
                    operand_stack.push_front(Self::parse_object(&inner, source)?);
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

                        let right = operand_stack.pop_front().ok_or_else(|| {
                            JqlError::new(JqlErrorKind::MissingOperand, token.span, source)
                        })?;
                        let left = operand_stack.pop_front().ok_or_else(|| {
                            JqlError::new(JqlErrorKind::MissingOperand, token.span, source)
                        })?;
                        let node = match op.kind {
                            JqlTokenKind::Pipe => JqlAstNode::pipe(left, right)?,
                            _ => JqlAstNode::binary(
                                JqlBinaryKind::from_token_kind(&op.kind)?,
                                left,
                                right,
                            )?,
                        };
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
                    // determine a function call
                    if let Some(next_token) = tokens.get(idx + 1) {
                        if next_token.kind == JqlTokenKind::LParen {
                            let mut depth = 1;
                            let mut end = idx + 2;
                            while end < tokens.len() {
                                match tokens[end].kind {
                                    JqlTokenKind::LParen => depth += 1,
                                    JqlTokenKind::RParen => {
                                        depth -= 1;
                                        if depth == 0 {
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                                end += 1;
                            }
                            if depth == 0 {
                                let arg_tokens = tokens[idx + 2..end].to_vec();
                                let arg = Self::parse(&arg_tokens, source)?;
                                operand_stack.push_front(JqlAstNode::call(
                                    &token,
                                    vec![arg],
                                    source,
                                )?);
                                idx = end + 1;
                            } else {
                                return Err(JqlError::new(
                                    JqlErrorKind::UnclosedParen,
                                    token.span,
                                    source,
                                ));
                            }
                        } else {
                            idx += 1;
                            operand_stack.push_front(JqlAstNode::identifier(&token, source)?);
                        }
                    } else {
                        idx += 1;
                        operand_stack.push_front(JqlAstNode::identifier(&token, source)?);
                    }
                }
                JqlTokenKind::Pipe
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
                            let right = operand_stack.pop_front().ok_or_else(|| {
                                JqlError::new(JqlErrorKind::MissingOperand, token.span, source)
                            })?;
                            let left = operand_stack.pop_front().ok_or_else(|| {
                                JqlError::new(JqlErrorKind::MissingOperand, token.span, source)
                            })?;

                            let node = match top.kind {
                                JqlTokenKind::Pipe => JqlAstNode::pipe(left, right)?,
                                _ => JqlAstNode::binary(
                                    JqlBinaryKind::from_token_kind(&top.kind)?,
                                    left,
                                    right,
                                )?,
                            };
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
                    return Err(JqlError::new(
                        JqlErrorKind::UnexpectedToken {
                            expected: ExpectedSyntax::Expression,
                            found: other,
                        },
                        token.span,
                        source,
                    ));
                }
            }
        }

        while let Some(top_token) = operator_stack.pop_front() {
            let right = operand_stack.pop_front().ok_or_else(|| {
                JqlError::new(JqlErrorKind::MissingOperand, top_token.span, source)
            })?;
            let left = operand_stack.pop_front().ok_or_else(|| {
                JqlError::new(JqlErrorKind::MissingOperand, top_token.span, source)
            })?;
            let node = match top_token.kind {
                JqlTokenKind::Pipe => JqlAstNode::pipe(left, right)?,
                _ => {
                    JqlAstNode::binary(JqlBinaryKind::from_token_kind(&top_token.kind)?, left, right)?
                }
            };
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
