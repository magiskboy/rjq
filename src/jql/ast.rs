use crate::jql::{
    error::{ExpectedSyntax, JqlError, JqlErrorKind},
    token::{JqlToken, JqlTokenKind},
};

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd)]
pub enum JqlBinaryKind {
    And,
    Or,
    Less,
    Greater,
    Equal,
    NotEqual,
    LessEqual,
    GreaterEqual,
}

impl JqlBinaryKind {
    pub fn from_token_kind(kind: &JqlTokenKind) -> Result<Self, JqlError> {
        match kind {
            JqlTokenKind::AndLogicalOp => Ok(Self::And),
            JqlTokenKind::OrLogicalOp => Ok(Self::Or),
            JqlTokenKind::LessOp => Ok(Self::Less),
            JqlTokenKind::GreaterOp => Ok(Self::Greater),
            JqlTokenKind::LessEqualOp => Ok(Self::LessEqual),
            JqlTokenKind::GreaterEqualOp => Ok(Self::GreaterEqual),
            JqlTokenKind::NotEqualOp => Ok(Self::NotEqual),
            JqlTokenKind::EqualOp => Ok(Self::Equal),
            _ => Err(JqlError::without_span(JqlErrorKind::UnexpectedToken {
                expected: ExpectedSyntax::BinaryOperator,
                found: *kind,
            })),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd)]
pub enum JqlNumericKind {
    Plus,
    Subtract,
    Multiply,
    Divide,
    Mod,
}

impl JqlNumericKind {
    pub fn from_token_kind(kind: &JqlTokenKind) -> Result<Self, JqlError> {
        match kind {
            JqlTokenKind::Plus => Ok(Self::Plus),
            JqlTokenKind::Subtract => Ok(Self::Subtract),
            JqlTokenKind::Multiply => Ok(Self::Multiply),
            JqlTokenKind::Divide => Ok(Self::Divide),
            JqlTokenKind::Mod => Ok(Self::Mod),
            _ => Err(JqlError::without_span(JqlErrorKind::UnexpectedToken {
                expected: ExpectedSyntax::BinaryOperator,
                found: *kind,
            })),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectKey<'a> {
    Ident(&'a str),
    String(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectField<'a> {
    pub key: ObjectKey<'a>,
    pub value: Box<JqlAstNode<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayIndex {
    Range { start: usize, end: usize },
    Index(usize),
    MultipleIndex(Vec<usize>),
    All,
}

/// One step in an access path. Identity `.` is an empty step list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessStep<'a> {
    Property(&'a str),
    Index(ArrayIndex),
    Project(Vec<&'a str>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfArm<'a> {
    pub cond: Box<JqlAstNode<'a>>,
    pub then_body: Box<JqlAstNode<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JqlAstNode<'a> {
    LParen,
    RParen,

    // Operator nodes
    Access(Vec<AccessStep<'a>>),
    Pipe {
        source: Box<JqlAstNode<'a>>,
        dest: Box<JqlAstNode<'a>>,
    },
    Call {
        name: &'a str,
        args: Vec<JqlAstNode<'a>>,
    },
    Binary {
        kind: JqlBinaryKind,
        left: Box<JqlAstNode<'a>>,
        right: Box<JqlAstNode<'a>>,
    },
    Numeric {
        kind: JqlNumericKind,
        left: Box<JqlAstNode<'a>>,
        right: Box<JqlAstNode<'a>>,
    },
    Object {
        fields: Vec<ObjectField<'a>>,
    },
    If {
        arms: Vec<IfArm<'a>>,
        else_body: Box<JqlAstNode<'a>>,
    },

    // Operand node
    Number(f64),
    String(&'a str),
    Boolean(bool),
    Null,
}

impl<'a> JqlAstNode<'a> {
    pub fn numeric(
        kind: JqlNumericKind,
        left: JqlAstNode<'a>,
        right: JqlAstNode<'a>,
    ) -> Result<JqlAstNode<'a>, JqlError> {
        Ok(JqlAstNode::Numeric {
            kind,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn binary(
        kind: JqlBinaryKind,
        left: JqlAstNode<'a>,
        right: JqlAstNode<'a>,
    ) -> Result<JqlAstNode<'a>, JqlError> {
        Ok(JqlAstNode::Binary {
            kind: kind,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn call(
        name: &JqlToken,
        args: Vec<JqlAstNode<'a>>,
        source: &'a str,
    ) -> Result<JqlAstNode<'a>, JqlError> {
        let err = JqlError::new(
            JqlErrorKind::UnexpectedToken {
                expected: ExpectedSyntax::Token(JqlTokenKind::Identifier),
                found: name.kind,
            },
            name.span,
            source,
        );
        Ok(JqlAstNode::Call::<'a> {
            name: source.get(name.span.start..name.span.end).ok_or(err)?,
            args: args,
        })
    }

    pub fn pipe(source: JqlAstNode<'a>, dest: JqlAstNode<'a>) -> Result<JqlAstNode<'a>, JqlError> {
        Ok(JqlAstNode::Pipe {
            source: Box::new(source),
            dest: Box::new(dest),
        })
    }

    pub fn object(fields: Vec<ObjectField<'a>>) -> Result<JqlAstNode<'a>, JqlError> {
        Ok(JqlAstNode::Object { fields })
    }

    pub fn if_expr(arms: Vec<IfArm<'a>>, else_body: JqlAstNode<'a>) -> JqlAstNode<'a> {
        JqlAstNode::If {
            arms,
            else_body: Box::new(else_body),
        }
    }

    pub fn object_key(token: &JqlToken, source: &'a str) -> Result<ObjectKey<'a>, JqlError> {
        match token.kind {
            JqlTokenKind::Identifier => Ok(ObjectKey::Ident(
                source.get(token.span.start..token.span.end).unwrap(),
            )),
            JqlTokenKind::String => Ok(ObjectKey::String(
                source
                    .get(token.span.start + 1..token.span.end - 1)
                    .unwrap(),
            )),
            other => Err(JqlError::new(
                JqlErrorKind::UnexpectedToken {
                    expected: ExpectedSyntax::Token(JqlTokenKind::Identifier),
                    found: other,
                },
                token.span,
                source,
            )),
        }
    }

    pub fn access(steps: Vec<AccessStep<'a>>) -> JqlAstNode<'a> {
        JqlAstNode::Access(steps)
    }

    pub fn number(token: &JqlToken, source: &'a str) -> Result<JqlAstNode<'a>, JqlError> {
        if token.kind == JqlTokenKind::Number {
            return Ok(JqlAstNode::Number(
                source
                    .get(token.span.start..token.span.end)
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
            ));
        }

        Err(JqlError::new(
            JqlErrorKind::UnexpectedToken {
                expected: ExpectedSyntax::Token(JqlTokenKind::Number),
                found: token.kind,
            },
            token.span,
            source,
        ))
    }

    pub fn string(token: &JqlToken, source: &'a str) -> Result<JqlAstNode<'a>, JqlError> {
        if token.kind == JqlTokenKind::String {
            return Ok(JqlAstNode::String(
                source
                    .get(token.span.start + 1..token.span.end - 1)
                    .unwrap(),
            ));
        }

        Err(JqlError::new(
            JqlErrorKind::UnexpectedToken {
                expected: ExpectedSyntax::Token(JqlTokenKind::String),
                found: token.kind,
            },
            token.span,
            source,
        ))
    }

    pub fn bool(token: &JqlToken, source: &'a str) -> Result<JqlAstNode<'a>, JqlError> {
        if token.kind == JqlTokenKind::Boolean {
            let value_str = source.get(token.span.start..token.span.end).unwrap();
            if value_str == "true" {
                return Ok(JqlAstNode::Boolean(true));
            }
            if value_str == "false" {
                return Ok(JqlAstNode::Boolean(false));
            }
        }

        Err(JqlError::new(
            JqlErrorKind::UnexpectedToken {
                expected: ExpectedSyntax::Token(JqlTokenKind::Boolean),
                found: token.kind,
            },
            token.span,
            source,
        ))
    }

    pub fn null(token: &JqlToken) -> Result<JqlAstNode<'a>, JqlError> {
        if token.kind == JqlTokenKind::Null {
            return Ok(JqlAstNode::Null);
        }
        Err(JqlError {
            kind: JqlErrorKind::UnexpectedToken {
                expected: ExpectedSyntax::Token(JqlTokenKind::Null),
                found: token.kind,
            },
            span: Some(token.span),
            location: None,
        })
    }
}
