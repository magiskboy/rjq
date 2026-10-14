## Grammar

```bnf
<pipeline> ::= <expression>
             | <expression> <space> "|" <space> <pipeline>

<expression> ::= <access>
               | <call>
               | <object>

<access> ::= <root_access> 
           | <access_step>+

<root_access> ::= "."

<access_step> ::= "." <ident>
                | "." "\"" <key_chars> "\""
                | "." "[" <number> "]"
                | "." "[" "\"" <key_chars> "\"" "]"

<call> ::= <identifier> "(" <expression> ")"

<filter> ::= "filter" "(" <condition> ")"

<map> ::= "map" "(" <expression> ")"

<object> ::= "{" "}"
           | "{" <object_field> ("," <object_field>)* "}"

<object_field> ::= <identifier> ":" <expression>
                 | <string_value> ":" <expression>

<condition> ::= <comparision>
              | "("+ <condition> ")"+ <space> <logical_op> <space> <comparision>

<logical_op> ::= "and" | "or"

<comparision> ::= <operand> <space> <com_op> <space> <operand>

<operand> ::= <access> | <literal>

<com_op> ::= "==" | ">=" | "<=" | ">" | "<" | "!="

<literal> ::= <string_value> | <boolean> | <number>

<number> ::= ["-"] <digits> ["." <digits>] [("e" | "E") ["+" | "-"] <digits>]

<boolean> ::= "true" | "false"

<ident> ::= ([a-z] | [A-Z] | "_") ([a-z] | [A-Z] | [0-9] | "_")*

<proj_key> ::= <ident> | <digits> | <string_value>

<null> ::= "null"

<space> ::= " "

<string_value> ::= "\"" <key_chars> "\""

<key_chars> ::= any chars except '"'
```

Grammar of access identifier

```
.jobs      -> List<Job>
.jobs[1]   -> Job
.jobs[]    -> Iterator<Job>
.jobs[1,2] -> Iterator<Job>
.jobs[1:3] -> Iterator<Job>

."kebab-case" / .["x-y"] / ."with space"  -> key with -, space, unicode
._id                                      -> underscore key

.person{}           -> Iterator<Pair>
.person{}@keys      -> Iterator<Pair[0]>
.person{}@values    -> Iterator<Pair[1]>
.person{name,age}   -> Iterator<Pair if Pair[0] in [name, age]>
.person{_id,name2,0,"a-b"} -> projection keys: _, digits, quoted

.jobs[]{}  -> Iterator<Job>
.jobs[]{title,description} -> Iterator<Pair if Pair[0] in [title, description]>
```

## Examples
```
.[1] | filter(.id > 10 && .age < 20 && (.money > 10 || .gold >= 1))
```

## Parser

```
Pipeline
├── Access
│   └── Index(1)
│
└── Call("filter")
    │
    └── And
        ├── And
        │   ├── GreaterThan
        │   │   ├── Access("id")
        │   │   └── 10
        │   │
        │   └── LessThan
        │       ├── Access("age")
        │       └── 20
        │
        └── Or
            ├── GreaterThan
            │   ├── Access("money")
            │   └── 10
            │
            └── GreaterThanOrEqual
                ├── Access("gold")
                └── 1
```
