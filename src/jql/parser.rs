use std::collections::VecDeque;

use crate::Span;
use crate::jql::ast::JqlNumericKind;
use crate::jql::{
    ast::{AccessStep, ArrayIndex, IfArm, JqlAstNode, JqlBinaryKind, ObjectField},
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

    /// Find the next branch delimiter at nesting depth 0 (parens / braces / brackets /
    /// nested `if`…`end`). Used to slice cond / then / else bodies for recursive parse.
    fn find_branch_delim(
        tokens: &[JqlToken],
        start: usize,
        delims: &[JqlTokenKind],
    ) -> Option<usize> {
        let mut paren = 0i32;
        let mut brace = 0i32;
        let mut bracket = 0i32;
        let mut if_depth = 0i32;

        for i in start..tokens.len() {
            let kind = tokens[i].kind;
            let at_surface = paren == 0 && brace == 0 && bracket == 0;

            match kind {
                JqlTokenKind::LParen => paren += 1,
                JqlTokenKind::RParen => paren -= 1,
                JqlTokenKind::LBrace => brace += 1,
                JqlTokenKind::RBrace => brace -= 1,
                JqlTokenKind::LBracket => bracket += 1,
                JqlTokenKind::RBracket => bracket -= 1,
                JqlTokenKind::If if at_surface => if_depth += 1,
                JqlTokenKind::End if at_surface => {
                    if if_depth == 0 {
                        if delims.contains(&JqlTokenKind::End) {
                            return Some(i);
                        }
                    } else {
                        if_depth -= 1;
                    }
                }
                kind if at_surface
                    && if_depth == 0
                    && kind != JqlTokenKind::End
                    && delims.contains(&kind) =>
                {
                    return Some(i);
                }
                _ => {}
            }
        }
        None
    }

    fn missing_branch_delim(
        expected: JqlTokenKind,
        tokens: &[JqlToken],
        start: usize,
        open_span: Span,
        source: &str,
    ) -> JqlError {
        if start >= tokens.len() {
            return Self::unexpected_token(
                ExpectedSyntax::Token(expected),
                JqlTokenKind::Stop,
                open_span,
                source,
            );
        }
        let last = tokens.last().copied().unwrap_or(tokens[start]);
        Self::unexpected_token(
            ExpectedSyntax::Token(expected),
            JqlTokenKind::Stop,
            last.span,
            source,
        )
    }

    fn parse_if<'a>(
        tokens: &[JqlToken],
        idx: usize,
        source: &'a str,
    ) -> Result<(JqlAstNode<'a>, usize), JqlError> {
        debug_assert_eq!(tokens[idx].kind, JqlTokenKind::If);
        let if_token = tokens[idx];
        let mut i = idx + 1;
        let mut arms = Vec::new();

        loop {
            let then_pos =
                Self::find_branch_delim(tokens, i, &[JqlTokenKind::Then]).ok_or_else(|| {
                    Self::missing_branch_delim(JqlTokenKind::Then, tokens, i, if_token.span, source)
                })?;
            if then_pos == i {
                return Err(Self::unexpected_token(
                    ExpectedSyntax::Expression,
                    JqlTokenKind::Then,
                    tokens[then_pos].span,
                    source,
                ));
            }
            let cond = Self::parse(&tokens[i..then_pos], source)?;
            i = then_pos + 1;

            let arm_end = Self::find_branch_delim(
                tokens,
                i,
                &[JqlTokenKind::Elif, JqlTokenKind::Else, JqlTokenKind::End],
            )
            .ok_or_else(|| {
                Self::missing_branch_delim(JqlTokenKind::Else, tokens, i, if_token.span, source)
            })?;
            if arm_end == i {
                return Err(Self::unexpected_token(
                    ExpectedSyntax::Expression,
                    tokens[arm_end].kind,
                    tokens[arm_end].span,
                    source,
                ));
            }
            if tokens[arm_end].kind == JqlTokenKind::End {
                return Err(Self::unexpected_token(
                    ExpectedSyntax::Token(JqlTokenKind::Else),
                    JqlTokenKind::End,
                    tokens[arm_end].span,
                    source,
                ));
            }

            let body = Self::parse(&tokens[i..arm_end], source)?;
            arms.push(IfArm {
                cond: Box::new(cond),
                then_body: Box::new(body),
            });

            match tokens[arm_end].kind {
                JqlTokenKind::Elif => {
                    i = arm_end + 1;
                }
                JqlTokenKind::Else => {
                    i = arm_end + 1;
                    break;
                }
                _ => unreachable!(),
            }
        }

        let end_pos =
            Self::find_branch_delim(tokens, i, &[JqlTokenKind::End]).ok_or_else(|| {
                Self::missing_branch_delim(JqlTokenKind::End, tokens, i, if_token.span, source)
            })?;
        if end_pos == i {
            return Err(Self::unexpected_token(
                ExpectedSyntax::Expression,
                JqlTokenKind::End,
                tokens[end_pos].span,
                source,
            ));
        }
        let else_body = Self::parse(&tokens[i..end_pos], source)?;
        Ok((JqlAstNode::if_expr(arms, else_body), end_pos + 1))
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

    fn slice<'a>(token: &JqlToken, source: &'a str) -> &'a str {
        source.get(token.span.start..token.span.end).unwrap()
    }

    fn string_content<'a>(token: &JqlToken, source: &'a str) -> &'a str {
        source
            .get(token.span.start + 1..token.span.end - 1)
            .unwrap()
    }

    fn adjacent(prev_end: usize, token: &JqlToken) -> bool {
        token.span.start == prev_end
    }

    fn parse_usize_index(token: &JqlToken, source: &str) -> Result<usize, JqlError> {
        let raw = Self::slice(token, source);
        if raw.contains('.') || raw.contains('e') || raw.contains('E') || raw.starts_with('-') {
            return Err(Self::unexpected_token(
                ExpectedSyntax::Token(JqlTokenKind::Number),
                token.kind,
                token.span,
                source,
            ));
        }
        raw.parse::<usize>().map_err(|_| {
            Self::unexpected_token(
                ExpectedSyntax::Token(JqlTokenKind::Number),
                token.kind,
                token.span,
                source,
            )
        })
    }

    fn parse_proj_key<'a>(token: &JqlToken, source: &'a str) -> Result<&'a str, JqlError> {
        match token.kind {
            JqlTokenKind::Identifier => Ok(Self::slice(token, source)),
            JqlTokenKind::String => Ok(Self::string_content(token, source)),
            JqlTokenKind::Number => {
                let raw = Self::slice(token, source);
                if raw.contains('.')
                    || raw.contains('e')
                    || raw.contains('E')
                    || raw.starts_with('-')
                {
                    return Err(Self::unexpected_token(
                        ExpectedSyntax::Token(JqlTokenKind::Identifier),
                        token.kind,
                        token.span,
                        source,
                    ));
                }
                Ok(raw)
            }
            other => Err(Self::unexpected_token(
                ExpectedSyntax::Token(JqlTokenKind::Identifier),
                other,
                token.span,
                source,
            )),
        }
    }

    fn parse_projection<'a>(
        tokens: &[JqlToken],
        idx: usize,
        source: &'a str,
    ) -> Result<(Option<AccessStep<'a>>, usize), JqlError> {
        // tokens[idx] is LBrace
        let open = tokens[idx];
        let Some(end) = Self::find_matching_close(
            tokens,
            idx + 1,
            JqlTokenKind::LBrace,
            JqlTokenKind::RBrace,
            1,
        ) else {
            return Err(JqlError::new(
                JqlErrorKind::UnclosedBrace,
                open.span,
                source,
            ));
        };

        let inner = &tokens[idx + 1..end];
        if inner.is_empty() {
            // `{}` is a no-op projection (same as PropertySelection::All before).
            return Ok((None, end + 1));
        }

        let mut keys = Vec::new();
        let mut i = 0;
        while i < inner.len() {
            keys.push(Self::parse_proj_key(&inner[i], source)?);
            i += 1;
            if i >= inner.len() {
                break;
            }
            if inner[i].kind != JqlTokenKind::Comma {
                return Err(Self::unexpected_token(
                    ExpectedSyntax::Token(JqlTokenKind::Comma),
                    inner[i].kind,
                    inner[i].span,
                    source,
                ));
            }
            i += 1;
            if i >= inner.len() {
                return Err(JqlError::new(
                    JqlErrorKind::InvalidExpression,
                    inner[i - 1].span,
                    source,
                ));
            }
        }

        Ok((Some(AccessStep::Project(keys)), end + 1))
    }

    fn parse_index_body<'a>(
        tokens: &[JqlToken],
        source: &'a str,
        open: JqlToken,
    ) -> Result<AccessStep<'a>, JqlError> {
        if tokens.is_empty() {
            return Ok(AccessStep::Index(ArrayIndex::All));
        }

        if tokens.len() == 1 && tokens[0].kind == JqlTokenKind::String {
            return Ok(AccessStep::Property(Self::string_content(
                &tokens[0], source,
            )));
        }

        if tokens.len() == 1 && tokens[0].kind == JqlTokenKind::Number {
            return Ok(AccessStep::Index(ArrayIndex::Index(
                Self::parse_usize_index(&tokens[0], source)?,
            )));
        }

        if tokens.len() == 3
            && tokens[0].kind == JqlTokenKind::Number
            && tokens[1].kind == JqlTokenKind::Colon
            && tokens[2].kind == JqlTokenKind::Number
        {
            let start = Self::parse_usize_index(&tokens[0], source)?;
            let end = Self::parse_usize_index(&tokens[2], source)?;
            return Ok(AccessStep::Index(ArrayIndex::Range { start, end }));
        }

        // Multi-index: number ("," number)+
        if tokens.len() >= 3 && tokens.len() % 2 == 1 {
            let mut indices = Vec::new();
            let mut i = 0;
            while i < tokens.len() {
                if tokens[i].kind != JqlTokenKind::Number {
                    break;
                }
                indices.push(Self::parse_usize_index(&tokens[i], source)?);
                i += 1;
                if i >= tokens.len() {
                    return Ok(AccessStep::Index(ArrayIndex::MultipleIndex(indices)));
                }
                if tokens[i].kind != JqlTokenKind::Comma {
                    break;
                }
                i += 1;
                if i >= tokens.len() {
                    return Err(JqlError::new(
                        JqlErrorKind::InvalidExpression,
                        tokens[i - 1].span,
                        source,
                    ));
                }
            }
            if i == tokens.len() && indices.len() >= 2 {
                return Ok(AccessStep::Index(ArrayIndex::MultipleIndex(indices)));
            }
        }

        Err(JqlError::new(
            JqlErrorKind::InvalidExpression,
            open.span,
            source,
        ))
    }

    fn parse_index_step<'a>(
        tokens: &[JqlToken],
        idx: usize,
        source: &'a str,
    ) -> Result<(AccessStep<'a>, usize), JqlError> {
        let open = tokens[idx];
        let Some(end) = Self::find_matching_close(
            tokens,
            idx + 1,
            JqlTokenKind::LBracket,
            JqlTokenKind::RBracket,
            1,
        ) else {
            return Err(Self::unexpected_token(
                ExpectedSyntax::Token(JqlTokenKind::RBracket),
                JqlTokenKind::Stop,
                open.span,
                source,
            ));
        };
        let step = Self::parse_index_body(&tokens[idx + 1..end], source, open)?;
        Ok((step, end + 1))
    }

    /// Parse an access starting at `tokens[idx]` (`Dot`). Returns the node and next index.
    /// Steps must be lexically adjacent (no whitespace between path tokens).
    fn parse_access<'a>(
        tokens: &[JqlToken],
        idx: usize,
        source: &'a str,
    ) -> Result<(JqlAstNode<'a>, usize), JqlError> {
        debug_assert_eq!(tokens[idx].kind, JqlTokenKind::Dot);
        let mut steps = Vec::new();
        let mut i = idx + 1;
        let mut cursor = tokens[idx].span.end;
        let mut after_project = false;

        while i < tokens.len() {
            let token = tokens[i];
            if !Self::adjacent(cursor, &token) {
                break;
            }

            if after_project {
                if token.kind != JqlTokenKind::Dot {
                    break;
                }
                i += 1;
                cursor = token.span.end;
                let Some(next) = tokens.get(i) else {
                    return Err(JqlError::new(
                        JqlErrorKind::InvalidExpression,
                        token.span,
                        source,
                    ));
                };
                if !Self::adjacent(cursor, next) {
                    return Err(JqlError::new(
                        JqlErrorKind::InvalidExpression,
                        token.span,
                        source,
                    ));
                }
                match next.kind {
                    JqlTokenKind::Identifier => {
                        steps.push(AccessStep::Property(Self::slice(next, source)));
                        cursor = next.span.end;
                        i += 1;
                        after_project = false;
                    }
                    JqlTokenKind::String => {
                        steps.push(AccessStep::Property(Self::string_content(next, source)));
                        cursor = next.span.end;
                        i += 1;
                        after_project = false;
                    }
                    JqlTokenKind::LBrace => {
                        let (proj, next_i) = Self::parse_projection(tokens, i, source)?;
                        cursor = tokens[next_i - 1].span.end;
                        i = next_i;
                        if let Some(step) = proj {
                            steps.push(step);
                            after_project = true;
                        } else {
                            after_project = false;
                        }
                    }
                    other => {
                        return Err(Self::unexpected_token(
                            ExpectedSyntax::Token(JqlTokenKind::Identifier),
                            other,
                            next.span,
                            source,
                        ));
                    }
                }
                continue;
            }

            match token.kind {
                JqlTokenKind::Identifier if steps.is_empty() => {
                    steps.push(AccessStep::Property(Self::slice(&token, source)));
                    cursor = token.span.end;
                    i += 1;
                }
                JqlTokenKind::String if steps.is_empty() => {
                    steps.push(AccessStep::Property(Self::string_content(&token, source)));
                    cursor = token.span.end;
                    i += 1;
                }
                JqlTokenKind::Dot => {
                    i += 1;
                    cursor = token.span.end;
                    let Some(next) = tokens.get(i) else {
                        return Err(JqlError::new(
                            JqlErrorKind::InvalidExpression,
                            token.span,
                            source,
                        ));
                    };
                    if !Self::adjacent(cursor, next) {
                        return Err(JqlError::new(
                            JqlErrorKind::InvalidExpression,
                            token.span,
                            source,
                        ));
                    }
                    match next.kind {
                        JqlTokenKind::Identifier => {
                            steps.push(AccessStep::Property(Self::slice(next, source)));
                            cursor = next.span.end;
                            i += 1;
                        }
                        JqlTokenKind::String => {
                            steps.push(AccessStep::Property(Self::string_content(next, source)));
                            cursor = next.span.end;
                            i += 1;
                        }
                        JqlTokenKind::LBrace => {
                            let (proj, next_i) = Self::parse_projection(tokens, i, source)?;
                            cursor = tokens[next_i - 1].span.end;
                            i = next_i;
                            if let Some(step) = proj {
                                steps.push(step);
                                after_project = true;
                            }
                        }
                        other => {
                            return Err(Self::unexpected_token(
                                ExpectedSyntax::Token(JqlTokenKind::Identifier),
                                other,
                                next.span,
                                source,
                            ));
                        }
                    }
                }
                JqlTokenKind::LBracket => {
                    let (step, next_i) = Self::parse_index_step(tokens, i, source)?;
                    cursor = tokens[next_i - 1].span.end;
                    i = next_i;
                    steps.push(step);
                }
                JqlTokenKind::LBrace => {
                    let (proj, next_i) = Self::parse_projection(tokens, i, source)?;
                    cursor = tokens[next_i - 1].span.end;
                    i = next_i;
                    if let Some(step) = proj {
                        steps.push(step);
                        after_project = true;
                    }
                }
                _ => break,
            }
        }

        Ok((JqlAstNode::access(steps), i))
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

                JqlTokenKind::Dot => {
                    let (node, next_idx) = Self::parse_access(tokens, idx, source)?;
                    operand_stack.push_front(node);
                    idx = next_idx;
                }

                JqlTokenKind::Identifier => {
                    let (node, next_idx) = Self::parse_call(tokens, idx, token, source)?;
                    operand_stack.push_front(node);
                    idx = next_idx;
                }

                JqlTokenKind::If => {
                    let (node, next_idx) = Self::parse_if(tokens, idx, source)?;
                    operand_stack.push_front(node);
                    idx = next_idx;
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
