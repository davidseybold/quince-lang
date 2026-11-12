# Patterns

```ebnf
pattern      = alt { "|" alt }
alt          = "_" | [ "-" ] literal [ ( ".." | "..=" ) [ "-" ] literal ] | name | NAME | type_path "." NAME | tuple_pat | variant_pat
tuple_pat    = "(" ")" | "(" pattern "," pattern { "," pattern } [ "," ] ")"
variant_pat  = type_path [ "(" ( pattern { "," pattern } | pfields ) [ "," ] ")" ]
pfields      = ".." | pfield { "," pfield } [ "," ".." ]
pfield       = name ":" [ pattern ]
```

r[pat.variant-fields]
- A variant's named fields are matched by name: `Shape.Rect(width: w, height: h)`.

r[pat.shorthand]
- A field name with nothing after the colon binds a variable of the same name: `Shape.Rect(width:, height:)`.

r[pat.rest]
- `..` matches the remaining fields: `Shape.Rect(width:, ..)`.

r[pat.all-fields]
- Every field is either named or covered by `..`.

r[pat.forms]
- Literals (`0`, `"GET"`), nested patterns (`Ok(Some(x))`) and alternatives (`Shape.Circle(..) | Shape.Point`) are allowed.

r[pat.alt-names]
- Every alternative binds the same names.

r[pat.negative]
- A numeric literal pattern or range bound may start with `-`, as in `-5 => ...`.

r[pat.const]
- A constant is a pattern too. A `NAME`, such as `MINZ_BUF_ERROR`, or a type constant, such as `Status.OK`, matches a value equal to it under `Eq`.

r[pat.const-exhaustive]
- Constants count like literals for exhaustiveness, and a lowercase `name` always binds a new variable.

r[pat.qualified-variant]
- A variant is always written with its enum's name, in patterns and in expressions alike: `Shape.Rect`, `LoadError.Orders`, `Command.Stop`.

r[pat.bare-variants]
- The prelude's `Some`, `None`, `Ok` and `Err` are the only bare variant names.

r[pat.struct]
- A struct is matched the way a variant is, by its type name and named fields: `User(name:, ..)`. Every rule above applies to it.
