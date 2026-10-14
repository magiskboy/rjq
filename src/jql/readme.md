## Grammar

```ebnf
(* ── Expressions ───────────────────────────────────────────────────── *)

expression = pipe ;

(* Right-associative binary layers; && is outside || (looser). *)
pipe       = and_expr
           | and_expr "|" pipe ;

and_expr   = or_expr
           | or_expr "&&" and_expr ;

or_expr    = comparison
           | comparison "||" or_expr ;

comparison = numeric
           | numeric comp_op comparison ;

numeric    = term
           | term add_op numeric ;

term       = factor
           | factor mul_op term ;

factor     = literal
           | access
           | call
           | object
           | "(" expression ")" ;

comp_op    = "==" | "!=" | ">" | ">=" | "<" | "<=" ;
add_op     = "+" | "-" ;
mul_op     = "*" | "/" | "%" ;

literal    = number | string | boolean | "null" ;

(* Bare name → call with no args; `name()` is invalid. *)
call       = name
           | name "(" expression ")" ;

object     = "{" [ field { "," field } ] "}" ;
field      = object_key ":" expression ;
object_key = name | string ;

(* ── Lexical forms ─────────────────────────────────────────────────── *)

name       = letter { letter | digit | "_" } ;
letter     = "a" … "z" | "A" … "Z" ;
digit      = "0" … "9" ;

boolean    = "true" | "false" ;

number     = [ "-" ] digits [ "." digits ] [ exponent ] ;
digits     = digit { digit } ;
exponent   = ( "e" | "E" ) [ "+" | "-" ] digits ;

string     = '"' { key_char } '"' ;
key_char   = (* any character except '"' *) ;

(* Access is parsed as structured steps (not one opaque token).
   Path tokens must be adjacent — no whitespace inside a chain. *)
access     = "." { step } ;                    (* "." alone → identity *)

step       = prop
           | index
           | projection
           | "." prop
           | "." projection ;

(* After a non-empty projection, only `.…` may continue. *)

prop       = prop_ident | string ;
prop_ident = ( letter | "_" ) { letter | digit | "_" } ;

index      = "[" index_spec "]" ;
index_spec =                                   (* empty → all elements *)
           | digits                            (* single index *)
           | digits ":" digits                 (* half-open [start:end) *)
           | digits "," digits { "," digits }  (* multiple indices *)
           | string ;                          (* bracket key → property *)

projection = "{" [ proj_key { "," proj_key } ] "}" ;
proj_key   = prop_ident | digits | string ;
```

### Precedence (low → high)

| Level | Operators | Notes |
|------|-----------|--------|
| 1 | `\|` | pipe |
| 2 | `&&` | binds looser than `\|\|` |
| 3 | `\|\|` | |
| 4 | `==` `!=` `>` `>=` `<` `<=` | |
| 5 | `+` `-` | |
| 6 | `*` `/` `%` | |

### Access path examples

```
.                              → root (identity)
.jobs                          → property
.jobs[1]                       → index
.jobs[]                        → all elements
.jobs[1,2]                     → multiple indices
.jobs[1:3]                     → range [1:3)
."kebab-case" / .["x-y"]       → quoted / bracket string key
._id                           → underscore property
.person{}                      → project all keys
.person{name,age}              → project selected keys
.person{_id,name2,0,"a-b"}     → projection keys: ident / digits / string
.jobs[]{title,description}     → index-all then project
.company.teams[0].name         → chained steps
```

## Examples

```
.[1] | filter(.id > 10 && .age < 20 && (.money > 10 || .gold >= 1))
.jobs | filter(.active == true) | map({ name: .title, pay: .money })
```
