use crate::{
    Source, Span,
    jql::{
        error::{JqlError, JqlErrorKind},
        token::{JqlToken, JqlTokenKind},
    },
};

#[derive(Debug, Clone)]
pub struct JqlLexer<'a> {
    pub index: usize,
    pub source: Source<'a>,
}

#[derive(Debug, Clone, PartialEq, Copy)]
enum JqlLexerState {
    Start,

    // Delimiters
    InLBrace,
    InRBrace,
    InLeftBrace,
    InRightBrace,
    InLBracket,
    InRBracket,
    InColon,
    InComma,
    InDot,
    InPipe,

    // Literals / keywords (prefix states)
    InF,
    InFa,
    InFal,
    InFals,
    InFalse,
    InT,
    InTr,
    InTru,
    InTrue,
    InTh,
    InThe,
    InThen,
    InN,
    InNu,
    InNul,
    InNull,
    InI,
    InIf,
    InE,
    InEl,
    InEli,
    InElif,
    InEls,
    InElse,
    InEn,
    InEnd,
    InNumber,
    InNumberDot,
    InNumberFrac,
    InNumberExp,
    InNumberExpSign,
    InNumberExpDigit,
    OpenString,
    InString,
    CloseString,

    // Identifiers
    InIdentifier,

    // Operators
    StartEqual,
    InEqual,
    StartNotEqual,
    InNotEqual,
    InGreater,
    InLess,
    InGreaterEqual,
    InLessEqual,
    StartAnd,
    InAnd,
    InOr,
    InPlus,
    InSubtract,
    InMultiply,
    InDivide,
    InMod,

    Whitespace,
    InvalidToken,
}

impl<'a> JqlLexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            index: 0,
            source: Source::new(source),
        }
    }

    /// Alphanumeric continuation used while matching keyword prefixes
    /// (`true` / `false` / `null` / `if` / `then` / `elif` / `else` / `end`).
    fn is_keyword_ident_char(c: char) -> bool {
        matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9')
    }

    fn continue_keyword(expected: char, next: JqlLexerState, c: char) -> JqlLexerState {
        if c == expected {
            next
        } else if Self::is_keyword_ident_char(c) {
            JqlLexerState::InIdentifier
        } else {
            JqlLexerState::InvalidToken
        }
    }

    fn keyword_tail_or_ident(c: char) -> JqlLexerState {
        if Self::is_keyword_ident_char(c) {
            JqlLexerState::InIdentifier
        } else {
            JqlLexerState::InvalidToken
        }
    }

    fn transition_table(&self, state: &JqlLexerState, c: char) -> Option<JqlLexerState> {
        let next_state = match state {
            JqlLexerState::Start => match c {
                '"' => JqlLexerState::OpenString,
                't' => JqlLexerState::InT,
                'f' => JqlLexerState::InF,
                'n' => JqlLexerState::InN,
                'i' => JqlLexerState::InI,
                'e' => JqlLexerState::InE,
                '&' => JqlLexerState::StartAnd,
                '!' => JqlLexerState::StartNotEqual,
                '=' => JqlLexerState::StartEqual,
                '>' => JqlLexerState::InGreater,
                '<' => JqlLexerState::InLess,
                '+' => JqlLexerState::InPlus,
                '-' => JqlLexerState::InSubtract,
                '*' => JqlLexerState::InMultiply,
                '/' => JqlLexerState::InDivide,
                '%' => JqlLexerState::InMod,
                '|' => JqlLexerState::InPipe,
                '(' => JqlLexerState::InLBrace,
                ')' => JqlLexerState::InRBrace,
                '{' => JqlLexerState::InLeftBrace,
                '}' => JqlLexerState::InRightBrace,
                '[' => JqlLexerState::InLBracket,
                ']' => JqlLexerState::InRBracket,
                ':' => JqlLexerState::InColon,
                ',' => JqlLexerState::InComma,
                '0'..='9' => JqlLexerState::InNumber,
                '.' => JqlLexerState::InDot,
                'a'..='z' | 'A'..='Z' | '_' => JqlLexerState::InIdentifier,
                ' ' => JqlLexerState::Whitespace,
                _ => JqlLexerState::InvalidToken,
            },

            JqlLexerState::InPipe => match c {
                '|' => JqlLexerState::InOr,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InNumber => match c {
                '0'..='9' => JqlLexerState::InNumber,
                '.' => JqlLexerState::InNumberDot,
                'e' | 'E' => JqlLexerState::InNumberExp,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InNumberDot => match c {
                '0'..='9' => JqlLexerState::InNumberFrac,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InNumberFrac => match c {
                '0'..='9' => JqlLexerState::InNumberFrac,
                'e' | 'E' => JqlLexerState::InNumberExp,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InNumberExp => match c {
                '+' | '-' => JqlLexerState::InNumberExpSign,
                '0'..='9' => JqlLexerState::InNumberExpDigit,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InNumberExpSign => match c {
                '0'..='9' => JqlLexerState::InNumberExpDigit,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InNumberExpDigit => match c {
                '0'..='9' => JqlLexerState::InNumberExpDigit,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::OpenString | JqlLexerState::InString => match c {
                '"' => JqlLexerState::CloseString,
                _ => JqlLexerState::InString,
            },

            // `true` / `then`
            JqlLexerState::InT => match c {
                'r' => JqlLexerState::InTr,
                'h' => JqlLexerState::InTh,
                c if Self::is_keyword_ident_char(c) => JqlLexerState::InIdentifier,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InTr => Self::continue_keyword('u', JqlLexerState::InTru, c),
            JqlLexerState::InTru => Self::continue_keyword('e', JqlLexerState::InTrue, c),
            JqlLexerState::InTrue => Self::keyword_tail_or_ident(c),
            JqlLexerState::InTh => Self::continue_keyword('e', JqlLexerState::InThe, c),
            JqlLexerState::InThe => Self::continue_keyword('n', JqlLexerState::InThen, c),
            JqlLexerState::InThen => Self::keyword_tail_or_ident(c),

            JqlLexerState::InF => Self::continue_keyword('a', JqlLexerState::InFa, c),
            JqlLexerState::InFa => Self::continue_keyword('l', JqlLexerState::InFal, c),
            JqlLexerState::InFal => Self::continue_keyword('s', JqlLexerState::InFals, c),
            JqlLexerState::InFals => Self::continue_keyword('e', JqlLexerState::InFalse, c),
            JqlLexerState::InFalse => Self::keyword_tail_or_ident(c),

            JqlLexerState::InN => Self::continue_keyword('u', JqlLexerState::InNu, c),
            JqlLexerState::InNu => Self::continue_keyword('l', JqlLexerState::InNul, c),
            JqlLexerState::InNul => Self::continue_keyword('l', JqlLexerState::InNull, c),
            JqlLexerState::InNull => Self::keyword_tail_or_ident(c),

            // `if`
            JqlLexerState::InI => Self::continue_keyword('f', JqlLexerState::InIf, c),
            JqlLexerState::InIf => Self::keyword_tail_or_ident(c),

            // `elif` / `else` / `end`
            JqlLexerState::InE => match c {
                'l' => JqlLexerState::InEl,
                'n' => JqlLexerState::InEn,
                c if Self::is_keyword_ident_char(c) => JqlLexerState::InIdentifier,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InEl => match c {
                'i' => JqlLexerState::InEli,
                's' => JqlLexerState::InEls,
                c if Self::is_keyword_ident_char(c) => JqlLexerState::InIdentifier,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InEli => Self::continue_keyword('f', JqlLexerState::InElif, c),
            JqlLexerState::InElif => Self::keyword_tail_or_ident(c),
            JqlLexerState::InEls => Self::continue_keyword('e', JqlLexerState::InElse, c),
            JqlLexerState::InElse => Self::keyword_tail_or_ident(c),
            JqlLexerState::InEn => Self::continue_keyword('d', JqlLexerState::InEnd, c),
            JqlLexerState::InEnd => Self::keyword_tail_or_ident(c),

            JqlLexerState::InIdentifier => match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => JqlLexerState::InIdentifier,
                _ => JqlLexerState::InvalidToken,
            },

            JqlLexerState::StartEqual => match c {
                '=' => JqlLexerState::InEqual,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::StartNotEqual => match c {
                '=' => JqlLexerState::InNotEqual,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InGreater => match c {
                '=' => JqlLexerState::InGreaterEqual,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::InLess => match c {
                '=' => JqlLexerState::InLessEqual,
                _ => JqlLexerState::InvalidToken,
            },
            JqlLexerState::StartAnd => match c {
                '&' => JqlLexerState::InAnd,
                _ => JqlLexerState::InvalidToken,
            },
            // `-` alone is Subtract; `-` + digit continues as a signed Number.
            JqlLexerState::InSubtract => match c {
                '0'..='9' => JqlLexerState::InNumber,
                _ => JqlLexerState::InvalidToken,
            },

            _ => JqlLexerState::InvalidToken,
        };

        match next_state {
            JqlLexerState::InvalidToken => None,
            _ => Some(next_state),
        }
    }

    fn accept_state(&self, state: &JqlLexerState, start: usize, end: usize) -> Option<JqlToken> {
        let kind = match state {
            // Incomplete keyword prefixes are bare identifiers (`f`, `n`, `tru`, `el`, …).
            JqlLexerState::InF
            | JqlLexerState::InFa
            | JqlLexerState::InFal
            | JqlLexerState::InFals
            | JqlLexerState::InT
            | JqlLexerState::InTr
            | JqlLexerState::InTru
            | JqlLexerState::InTh
            | JqlLexerState::InThe
            | JqlLexerState::InN
            | JqlLexerState::InNu
            | JqlLexerState::InNul
            | JqlLexerState::InI
            | JqlLexerState::InE
            | JqlLexerState::InEl
            | JqlLexerState::InEli
            | JqlLexerState::InEls
            | JqlLexerState::InEn => {
                return Some(JqlToken::new(JqlTokenKind::Identifier, start, end));
            }

            JqlLexerState::InLBrace => JqlTokenKind::LParen,
            JqlLexerState::InRBrace => JqlTokenKind::RParen,
            JqlLexerState::InLeftBrace => JqlTokenKind::LBrace,
            JqlLexerState::InRightBrace => JqlTokenKind::RBrace,
            JqlLexerState::InLBracket => JqlTokenKind::LBracket,
            JqlLexerState::InRBracket => JqlTokenKind::RBracket,
            JqlLexerState::InColon => JqlTokenKind::Colon,
            JqlLexerState::InComma => JqlTokenKind::Comma,
            JqlLexerState::InDot => JqlTokenKind::Dot,
            JqlLexerState::InPipe => JqlTokenKind::Pipe,
            JqlLexerState::InFalse | JqlLexerState::InTrue => JqlTokenKind::Boolean,
            JqlLexerState::InNull => JqlTokenKind::Null,
            JqlLexerState::InIf => JqlTokenKind::If,
            JqlLexerState::InThen => JqlTokenKind::Then,
            JqlLexerState::InElif => JqlTokenKind::Elif,
            JqlLexerState::InElse => JqlTokenKind::Else,
            JqlLexerState::InEnd => JqlTokenKind::End,
            JqlLexerState::InNumber
            | JqlLexerState::InNumberFrac
            | JqlLexerState::InNumberExpDigit => JqlTokenKind::Number,
            JqlLexerState::CloseString => JqlTokenKind::String,
            JqlLexerState::InIdentifier => JqlTokenKind::Identifier,
            JqlLexerState::InEqual => JqlTokenKind::EqualOp,
            JqlLexerState::InNotEqual => JqlTokenKind::NotEqualOp,
            JqlLexerState::InGreater => JqlTokenKind::GreaterOp,
            JqlLexerState::InLess => JqlTokenKind::LessOp,
            JqlLexerState::InGreaterEqual => JqlTokenKind::GreaterEqualOp,
            JqlLexerState::InLessEqual => JqlTokenKind::LessEqualOp,
            JqlLexerState::InAnd => JqlTokenKind::AndLogicalOp,
            JqlLexerState::InOr => JqlTokenKind::OrLogicalOp,
            JqlLexerState::InPlus => JqlTokenKind::Plus,
            JqlLexerState::InSubtract => JqlTokenKind::Subtract,
            JqlLexerState::InMultiply => JqlTokenKind::Multiply,
            JqlLexerState::InDivide => JqlTokenKind::Divide,
            JqlLexerState::InMod => JqlTokenKind::Mod,
            JqlLexerState::Whitespace => JqlTokenKind::Whitespace,
            _ => JqlTokenKind::InvalidToken,
        };

        match kind {
            JqlTokenKind::InvalidToken => None,
            _ => Some(JqlToken::new(kind, start, end)),
        }
    }

    fn next_token(&mut self) -> Result<JqlToken, JqlError> {
        if self.index == self.source.len() {
            return Ok(JqlToken::new(JqlTokenKind::Stop, self.index, self.index));
        }

        let start = self.index;
        let mut state = JqlLexerState::Start;
        let mut position = self.index;
        let mut c = ' ';
        let mut last_accept_token: Option<JqlToken> = None;
        let mut last_accept_position = position;

        while position < self.source.len() {
            c = self.source.char_at(position).unwrap();

            let Some(next_state) = self.transition_table(&state, c) else {
                break;
            };

            let end = position + c.len_utf8();
            if let Some(token) = self.accept_state(&next_state, self.index, end) {
                last_accept_token = Some(token);
                last_accept_position = end;
            }

            state = next_state;
            position = end;
        }

        if let Some(token) = last_accept_token {
            self.index = last_accept_position;
            if token.kind == JqlTokenKind::Whitespace {
                return self.next_token();
            }
            return Ok(token);
        }

        self.index = position + c.len_utf8();
        Err(JqlError::new(
            JqlErrorKind::InvalidToken,
            Span {
                start,
                end: self.index,
            },
            self.source.data,
        ))
    }

    pub fn tokenize(source: &str) -> Result<Vec<JqlToken>, JqlError> {
        let mut lexer = JqlLexer::new(source);
        let mut tokens = Vec::new();

        loop {
            let token = lexer.next_token()?;
            if token.kind == JqlTokenKind::Stop {
                break;
            }
            tokens.push(token);
        }

        Ok(tokens)
    }
}

#[cfg(test)]
#[path = "../../tests/jql/lexer.rs"]
mod test;
