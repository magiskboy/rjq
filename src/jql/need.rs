//! Derive a JSON [`Need`](crate::json::lazy::Need) from a JQL AST (expression first).

use crate::jql::ast::{AccessStep, ArrayIndex, JqlAstNode};
use crate::json::builder::lazy::{Need, merge_need};

/// What shape of *its input* does this expression need?
pub fn required_input(ast: &JqlAstNode<'_>) -> Need {
    match ast {
        JqlAstNode::Null
        | JqlAstNode::Boolean(_)
        | JqlAstNode::Number(_)
        | JqlAstNode::String(_) => Need::empty(),
        JqlAstNode::Access(steps) if steps.is_empty() => Need::Full,
        JqlAstNode::Access(steps) => steps_to_need(steps, Need::Full),
        JqlAstNode::Pipe { source, dest } => compose(source, required_input(dest)),
        JqlAstNode::Call { name, args } => call_need(name, args),
        JqlAstNode::Binary { left, right, .. } => {
            merge_need(required_input(left), required_input(right))
        }
        JqlAstNode::Numeric { left, right, .. } => {
            merge_need(required_input(left), required_input(right))
        }
        JqlAstNode::Object { fields } => fields.iter().fold(Need::empty(), |acc, f| {
            merge_need(acc, required_input(&f.value))
        }),
        JqlAstNode::If { arms, else_body } => {
            let mut need = required_input(else_body);
            for arm in arms {
                need = merge_need(
                    need,
                    merge_need(required_input(&arm.cond), required_input(&arm.then_body)),
                );
            }
            need
        }
        JqlAstNode::LParen | JqlAstNode::RParen => Need::Full,
    }
}

fn steps_to_need(steps: &[AccessStep<'_>], leaf: Need) -> Need {
    let mut need = leaf;
    for step in steps.iter().rev() {
        need = match step {
            AccessStep::Property(key) => Need::field(*key, need),
            AccessStep::Project(keys) => {
                let key_need = if need.is_empty() || need.is_full() {
                    Need::Full
                } else {
                    need.clone()
                };
                keys.iter().fold(Need::empty(), |acc, k| {
                    merge_need(acc, Need::field(*k, key_need.clone()))
                })
            }
            AccessStep::Index(ArrayIndex::All) => Need::array_all(need),
            AccessStep::Index(ArrayIndex::Index(i)) => Need::array_index(*i, need),
            AccessStep::Index(ArrayIndex::MultipleIndex(indices)) => {
                indices.iter().fold(Need::empty(), |acc, i| {
                    merge_need(acc, Need::array_index(*i, need.clone()))
                })
            }
            AccessStep::Index(ArrayIndex::Range { start, end }) => {
                (*start..*end).fold(Need::empty(), |acc, i| {
                    merge_need(acc, Need::array_index(i, need.clone()))
                })
            }
        };
    }
    need
}

fn call_need(name: &str, args: &[JqlAstNode<'_>]) -> Need {
    match name {
        // Output is the original element(s) — must materialize them fully.
        "filter" | "find" if args.len() == 1 => Need::array_all(Need::Full),
        // Output is a boolean — only predicate fields are required.
        "any" | "all" if args.len() == 1 => Need::array_all(required_input(&args[0])),
        "map" if args.len() == 1 => Need::array_all(required_input(&args[0])),
        "has" if args.len() == 1 => match &args[0] {
            JqlAstNode::String(key) => Need::field(*key, Need::Full),
            other => merge_need(Need::Full, required_input(other)),
        },
        "contains" | "startswith" | "endswith" | "join" | "split" if args.len() == 1 => {
            merge_need(Need::Full, required_input(&args[0]))
        }
        "len" | "sum" | "avg" | "min" | "max" | "first" | "last" | "keys" | "values" | "type"
        | "reverse" | "flatten" | "unique" | "sort" | "not" | "abs" | "empty" => Need::Full,
        _ => Need::Full,
    }
}

fn compose(source: &JqlAstNode<'_>, dest_need: Need) -> Need {
    if let JqlAstNode::Access(steps) = source
        && steps.is_empty()
    {
        return dest_need;
    }

    match source {
        JqlAstNode::Access(steps) => steps_to_need(steps, dest_need),
        JqlAstNode::Call { name, args }
            if matches!(*name, "filter" | "find") && args.len() == 1 =>
        {
            merge_need(Need::array_all(required_input(&args[0])), dest_need)
        }
        JqlAstNode::Call { name, args } if *name == "map" && args.len() == 1 => {
            if dest_need.is_full() || dest_need.is_empty() {
                Need::array_all(required_input(&args[0]))
            } else {
                // Cannot invert map's element transform safely.
                Need::array_all(Need::Full)
            }
        }
        JqlAstNode::Pipe {
            source: inner,
            dest,
        } => compose(inner, compose(dest, dest_need)),
        other => {
            let base = required_input(other);
            if dest_need.is_empty() || dest_need.is_full() {
                base
            } else {
                base
            }
        }
    }
}
