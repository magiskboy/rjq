//! Peephole AST optimizer: rewrite bottom-up until a fixed point.
//!
//! | Kind | Before | After |
//! |------|--------|-------|
//! | Identity pipe | `. \| x` / `x \| .` | `x` |
//! | Access fuse | `.a \| .b` | `.a.b` |
//! | Access fuse (chain) | `.a \| (.b \| rest)` | `.a.b \| rest` |
//! | Filter fuse | `filter(p) \| filter(q)` | `filter(p && q)` |
//! | Filter fuse (chain) | `filter(p) \| (filter(q) \| rest)` | `filter(p && q) \| rest` |
//! | Map fuse | `map(f) \| map(g)` | `map(f \| g)` |
//! | Map fuse (chain) | `map(f) \| (map(g) \| rest)` | `map(f \| g) \| rest` |
//! | Numeric fold | `1 + 2 * 3` | `7` |
//! | Comparison fold | `1 < 2` / `"a" == "a"` | `true` / `true` |
//! | And absorb | `true && x` / `x && true` | `x` |
//! | And short-circuit | `false && x` | `false` |
//! | Or absorb | `false \|\| x` / `x \|\| false` | `x` |
//! | Or short-circuit | `true \|\| x` | `true` |
//! | If const-true | `if true then a else b end` | `a` |
//! | If const-false | `if false then a else b end` | `b` |
//! | If dead arm | `if false then a elif c then d else e end` | `if c then d else e end` |
//!
//! Rules apply through right-associative pipe chains. Division/mod by zero
//! and non-literal operands are left unchanged.

use crate::jql::ast::{IfArm, JqlAstNode, JqlBinaryKind, JqlNumericKind, ObjectField};

/// Run the peephole rewrite table above until the AST stops changing.
pub fn optimize<'a>(ast: JqlAstNode<'a>) -> JqlAstNode<'a> {
    let mut current = ast;
    loop {
        let next = rewrite(current.clone());
        if next == current {
            return next;
        }
        current = next;
    }
}

fn rewrite<'a>(node: JqlAstNode<'a>) -> JqlAstNode<'a> {
    let node = rewrite_children(node);
    rewrite_node(node)
}

fn rewrite_children<'a>(node: JqlAstNode<'a>) -> JqlAstNode<'a> {
    match node {
        JqlAstNode::Pipe { source, dest } => JqlAstNode::Pipe {
            source: Box::new(rewrite(*source)),
            dest: Box::new(rewrite(*dest)),
        },
        JqlAstNode::Call { name, args } => JqlAstNode::Call {
            name,
            args: args.into_iter().map(rewrite).collect(),
        },
        JqlAstNode::Binary { kind, left, right } => JqlAstNode::Binary {
            kind,
            left: Box::new(rewrite(*left)),
            right: Box::new(rewrite(*right)),
        },
        JqlAstNode::Numeric { kind, left, right } => JqlAstNode::Numeric {
            kind,
            left: Box::new(rewrite(*left)),
            right: Box::new(rewrite(*right)),
        },
        JqlAstNode::Object { fields } => JqlAstNode::Object {
            fields: fields
                .into_iter()
                .map(|f| ObjectField {
                    key: f.key,
                    value: Box::new(rewrite(*f.value)),
                })
                .collect(),
        },
        JqlAstNode::If { arms, else_body } => JqlAstNode::If {
            arms: arms
                .into_iter()
                .map(|arm| IfArm {
                    cond: Box::new(rewrite(*arm.cond)),
                    then_body: Box::new(rewrite(*arm.then_body)),
                })
                .collect(),
            else_body: Box::new(rewrite(*else_body)),
        },
        other => other,
    }
}

fn rewrite_node<'a>(node: JqlAstNode<'a>) -> JqlAstNode<'a> {
    match node {
        // `. | x` / `x | .`
        JqlAstNode::Pipe { source, dest } if is_identity(&source) => *dest,
        JqlAstNode::Pipe { source, dest } if is_identity(&dest) => *source,

        // Pipe fusions (also through right-assoc `a | (b | c)` chains)
        JqlAstNode::Pipe {
            source,
            dest: dest_box,
        } => rewrite_pipe(*source, *dest_box),

        // Constant-fold arithmetic on number literals
        JqlAstNode::Numeric { kind, left, right } => match (*left, *right) {
            (JqlAstNode::Number(l), JqlAstNode::Number(r)) => match fold_numeric(kind, l, r) {
                Some(n) => JqlAstNode::Number(n),
                None => JqlAstNode::Numeric {
                    kind,
                    left: Box::new(JqlAstNode::Number(l)),
                    right: Box::new(JqlAstNode::Number(r)),
                },
            },
            (left, right) => JqlAstNode::Numeric {
                kind,
                left: Box::new(left),
                right: Box::new(right),
            },
        },

        // Boolean simplify + constant-fold comparisons on literals
        JqlAstNode::Binary { kind, left, right } => rewrite_binary(kind, *left, *right),

        // Dead-branch `if`
        JqlAstNode::If { arms, else_body } => rewrite_if(arms, *else_body),

        other => other,
    }
}

fn is_identity(node: &JqlAstNode<'_>) -> bool {
    matches!(node, JqlAstNode::Access(steps) if steps.is_empty())
}

fn rewrite_pipe<'a>(source: JqlAstNode<'a>, dest: JqlAstNode<'a>) -> JqlAstNode<'a> {
    match (source, dest) {
        // `Access(a) | Access(b)`
        (JqlAstNode::Access(mut a), JqlAstNode::Access(b)) => {
            a.extend(b);
            JqlAstNode::Access(a)
        }

        // `Access(a) | (Access(b) | rest)`
        (
            JqlAstNode::Access(mut a),
            JqlAstNode::Pipe {
                source: mid,
                dest: rest,
            },
        ) if matches!(mid.as_ref(), JqlAstNode::Access(_)) => {
            if let JqlAstNode::Access(b) = *mid {
                a.extend(b);
                rewrite_pipe(JqlAstNode::Access(a), *rest)
            } else {
                unreachable!()
            }
        }

        // `filter(p) | filter(q)` → `filter(p && q)`
        (
            JqlAstNode::Call {
                name: "filter",
                args: mut p_args,
            },
            JqlAstNode::Call {
                name: "filter",
                args: mut q_args,
            },
        ) if p_args.len() == 1 && q_args.len() == 1 => JqlAstNode::Call {
            name: "filter",
            args: vec![JqlAstNode::Binary {
                kind: JqlBinaryKind::And,
                left: Box::new(p_args.remove(0)),
                right: Box::new(q_args.remove(0)),
            }],
        },

        // `filter(p) | (filter(q) | rest)`
        (
            JqlAstNode::Call {
                name: "filter",
                args: mut p_args,
            },
            JqlAstNode::Pipe {
                source: mid,
                dest: rest,
            },
        ) if matches!(
            mid.as_ref(),
            JqlAstNode::Call {
                name: "filter",
                args
            } if args.len() == 1
        ) && p_args.len() == 1 =>
        {
            let JqlAstNode::Call {
                name: "filter",
                mut args,
            } = *mid
            else {
                unreachable!()
            };
            let fused = JqlAstNode::Call {
                name: "filter",
                args: vec![JqlAstNode::Binary {
                    kind: JqlBinaryKind::And,
                    left: Box::new(p_args.remove(0)),
                    right: Box::new(args.remove(0)),
                }],
            };
            rewrite_pipe(fused, *rest)
        }

        // `map(f) | map(g)` → `map(f | g)`
        (
            JqlAstNode::Call {
                name: "map",
                args: mut f_args,
            },
            JqlAstNode::Call {
                name: "map",
                args: mut g_args,
            },
        ) if f_args.len() == 1 && g_args.len() == 1 => JqlAstNode::Call {
            name: "map",
            args: vec![JqlAstNode::Pipe {
                source: Box::new(f_args.remove(0)),
                dest: Box::new(g_args.remove(0)),
            }],
        },

        // `map(f) | (map(g) | rest)`
        (
            JqlAstNode::Call {
                name: "map",
                args: mut f_args,
            },
            JqlAstNode::Pipe {
                source: mid,
                dest: rest,
            },
        ) if matches!(
            mid.as_ref(),
            JqlAstNode::Call {
                name: "map",
                args
            } if args.len() == 1
        ) && f_args.len() == 1 =>
        {
            let JqlAstNode::Call {
                name: "map",
                mut args,
            } = *mid
            else {
                unreachable!()
            };
            let fused = JqlAstNode::Call {
                name: "map",
                args: vec![JqlAstNode::Pipe {
                    source: Box::new(f_args.remove(0)),
                    dest: Box::new(args.remove(0)),
                }],
            };
            rewrite_pipe(fused, *rest)
        }

        (source, dest) => JqlAstNode::Pipe {
            source: Box::new(source),
            dest: Box::new(dest),
        },
    }
}

fn fold_numeric(kind: JqlNumericKind, l: f64, r: f64) -> Option<f64> {
    match kind {
        JqlNumericKind::Plus => Some(l + r),
        JqlNumericKind::Subtract => Some(l - r),
        JqlNumericKind::Multiply => Some(l * r),
        JqlNumericKind::Divide if r != 0.0 => Some(l / r),
        JqlNumericKind::Mod if r != 0.0 => {
            let li = l as i32;
            let ri = r as i32;
            if ri == 0 {
                None
            } else {
                Some((li % ri) as f64)
            }
        }
        _ => None,
    }
}

fn rewrite_binary<'a>(
    kind: JqlBinaryKind,
    left: JqlAstNode<'a>,
    right: JqlAstNode<'a>,
) -> JqlAstNode<'a> {
    match kind {
        // Absorb identity; fold only when the discarded side cannot raise
        // (literal false/true). Prefer short-circuit form: `false && x` /
        // `true || x` drop `x`; `x && false` / `x || true` keep `x` so
        // path errors on `x` are still observed.
        JqlBinaryKind::And => match (&left, &right) {
            (JqlAstNode::Boolean(true), _) => right,
            (_, JqlAstNode::Boolean(true)) => left,
            (JqlAstNode::Boolean(false), _) => JqlAstNode::Boolean(false),
            _ => JqlAstNode::Binary {
                kind,
                left: Box::new(left),
                right: Box::new(right),
            },
        },
        JqlBinaryKind::Or => match (&left, &right) {
            (JqlAstNode::Boolean(false), _) => right,
            (_, JqlAstNode::Boolean(false)) => left,
            (JqlAstNode::Boolean(true), _) => JqlAstNode::Boolean(true),
            _ => JqlAstNode::Binary {
                kind,
                left: Box::new(left),
                right: Box::new(right),
            },
        },
        _ => match fold_comparison(kind, &left, &right) {
            Some(b) => JqlAstNode::Boolean(b),
            None => JqlAstNode::Binary {
                kind,
                left: Box::new(left),
                right: Box::new(right),
            },
        },
    }
}

fn fold_comparison(
    kind: JqlBinaryKind,
    left: &JqlAstNode<'_>,
    right: &JqlAstNode<'_>,
) -> Option<bool> {
    match (left, right) {
        (JqlAstNode::Number(l), JqlAstNode::Number(r)) => Some(match kind {
            JqlBinaryKind::Equal => l == r,
            JqlBinaryKind::NotEqual => l != r,
            JqlBinaryKind::Less => l < r,
            JqlBinaryKind::LessEqual => l <= r,
            JqlBinaryKind::Greater => l > r,
            JqlBinaryKind::GreaterEqual => l >= r,
            _ => return None,
        }),
        (JqlAstNode::Boolean(l), JqlAstNode::Boolean(r)) => match kind {
            JqlBinaryKind::Equal => Some(l == r),
            JqlBinaryKind::NotEqual => Some(l != r),
            _ => None,
        },
        (JqlAstNode::String(l), JqlAstNode::String(r)) => match kind {
            JqlBinaryKind::Equal => Some(l == r),
            JqlBinaryKind::NotEqual => Some(l != r),
            _ => None,
        },
        (JqlAstNode::Null, JqlAstNode::Null) => match kind {
            JqlBinaryKind::Equal => Some(true),
            JqlBinaryKind::NotEqual => Some(false),
            _ => None,
        },
        (JqlAstNode::Null, _) | (_, JqlAstNode::Null) => match kind {
            JqlBinaryKind::Equal => Some(false),
            JqlBinaryKind::NotEqual => Some(true),
            _ => None,
        },
        _ => None,
    }
}

fn rewrite_if<'a>(arms: Vec<IfArm<'a>>, else_body: JqlAstNode<'a>) -> JqlAstNode<'a> {
    let mut kept = Vec::with_capacity(arms.len());
    for arm in arms {
        match *arm.cond {
            JqlAstNode::Boolean(true) => {
                // First constantly-true arm wins; drop the rest.
                if kept.is_empty() {
                    return *arm.then_body;
                }
                // Unreachable after a prior non-const arm; keep as else via then.
                return JqlAstNode::If {
                    arms: kept,
                    else_body: arm.then_body,
                };
            }
            JqlAstNode::Boolean(false) => {
                // Dead arm — skip.
            }
            other => kept.push(IfArm {
                cond: Box::new(other),
                then_body: arm.then_body,
            }),
        }
    }
    if kept.is_empty() {
        else_body
    } else {
        JqlAstNode::If {
            arms: kept,
            else_body: Box::new(else_body),
        }
    }
}
