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

<access_step> ::= "." <string>
                | "." "[" <number> "]"

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

<number> ::= [0-9]

<boolean> ::= "true" | "false"

<string> ::= [a-z] | [A-Z]

<null> ::= "null"

<space> ::= " "

<string_value> ::= "\"" ([a-z] | [A-Z] | [0-9] | " ") "\""
```

Grammar of access identifier

```
.jobs      -> List<Job>
.jobs[1]   -> Job
.jobs[]    -> Iterator<Job>
.jobs[1,2] -> Iterator<Job>
.jobs[1:3] -> Iterator<Job>

.person{}           -> Iterator<Pair>
.person{}@keys      -> Iterator<Pair[0]>
.person{}@values    -> Iterator<Pair[1]>
.person{name,age}   -> Iterator<Pair if Pair[0] in [name, age]>

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
