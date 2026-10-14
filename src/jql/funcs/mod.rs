mod abs;
mod all;
mod any;
mod avg;
mod contains;
mod empty;
mod endswith;
mod filter;
mod find;
mod first;
mod flatten;
mod has;
mod join;
mod keys;
mod last;
mod len;
mod map;
mod max;
mod min;
mod not;
mod reverse;
mod sort;
mod split;
mod startswith;
mod sum;
mod type_;
mod unique;
mod util;
mod values;

use crate::jql::{
    ast::JqlAstNode,
    error::JqlError,
    funcs::util::{expect_arity, op_not_support},
    proxy::Proxy,
};

pub fn dispatch<'a>(
    name: &str,
    args: &[JqlAstNode<'_>],
    value: Proxy<'a>,
) -> Result<Proxy<'a>, JqlError> {
    match name {
        // 1-arg
        "filter" => {
            expect_arity(name, args.len(), 1)?;
            filter::call(value, &args[0])
        }
        "map" => {
            expect_arity(name, args.len(), 1)?;
            map::call(value, &args[0])
        }
        "any" => {
            expect_arity(name, args.len(), 1)?;
            any::call(value, &args[0])
        }
        "all" => {
            expect_arity(name, args.len(), 1)?;
            all::call(value, &args[0])
        }
        "find" => {
            expect_arity(name, args.len(), 1)?;
            find::call(value, &args[0])
        }
        "has" => {
            expect_arity(name, args.len(), 1)?;
            has::call(value, &args[0])
        }
        "contains" => {
            expect_arity(name, args.len(), 1)?;
            contains::call(value, &args[0])
        }
        "join" => {
            expect_arity(name, args.len(), 1)?;
            join::call(value, &args[0])
        }
        "split" => {
            expect_arity(name, args.len(), 1)?;
            split::call(value, &args[0])
        }
        "startswith" => {
            expect_arity(name, args.len(), 1)?;
            startswith::call(value, &args[0])
        }
        "endswith" => {
            expect_arity(name, args.len(), 1)?;
            endswith::call(value, &args[0])
        }

        // 0-arg (act on current value)
        "len" => {
            expect_arity(name, args.len(), 0)?;
            len::call(value)
        }
        "sum" => {
            expect_arity(name, args.len(), 0)?;
            sum::call(value)
        }
        "avg" => {
            expect_arity(name, args.len(), 0)?;
            avg::call(value)
        }
        "min" => {
            expect_arity(name, args.len(), 0)?;
            min::call(value)
        }
        "max" => {
            expect_arity(name, args.len(), 0)?;
            max::call(value)
        }
        "first" => {
            expect_arity(name, args.len(), 0)?;
            first::call(value)
        }
        "last" => {
            expect_arity(name, args.len(), 0)?;
            last::call(value)
        }
        "keys" => {
            expect_arity(name, args.len(), 0)?;
            keys::call(value)
        }
        "values" => {
            expect_arity(name, args.len(), 0)?;
            values::call(value)
        }
        "type" => {
            expect_arity(name, args.len(), 0)?;
            type_::call(value)
        }
        "reverse" => {
            expect_arity(name, args.len(), 0)?;
            reverse::call(value)
        }
        "flatten" => {
            expect_arity(name, args.len(), 0)?;
            flatten::call(value)
        }
        "unique" => {
            expect_arity(name, args.len(), 0)?;
            unique::call(value)
        }
        "sort" => {
            expect_arity(name, args.len(), 0)?;
            sort::call(value)
        }
        "not" => {
            expect_arity(name, args.len(), 0)?;
            not::call(value)
        }
        "abs" => {
            expect_arity(name, args.len(), 0)?;
            abs::call(value)
        }
        "empty" => {
            expect_arity(name, args.len(), 0)?;
            empty::call(value)
        }

        other => Err(op_not_support(other)),
    }
}
