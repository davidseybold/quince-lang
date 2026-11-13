//! `SyntaxKind`: every kind of token and of CST node.

macro_rules! syntax_kinds {
    (
        tokens: [$($(#[$meta:meta])* $token:ident),* $(,)?],
        keywords: [$($kw:ident = $kw_text:literal),* $(,)?],
        punctuation: [$($punct:ident = $punct_text:literal),* $(,)?],
        joined: [$($joined:ident = $joined_text:literal),* $(,)?],
        nodes: [$($(#[$node_meta:meta])* $node:ident),* $(,)?] $(,)?
    ) => {
        /// The kind of a token or a CST node.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u16)]
        pub enum SyntaxKind {
            $($(#[$meta])* $token,)*
            $($kw,)*
            $($punct,)*
            $($joined,)*
            $($(#[$node_meta])* $node,)*
        }

        impl SyntaxKind {
            /// Every kind, in order, so `ALL[kind as usize] == kind`.
            pub const ALL: &[SyntaxKind] = &[
                $(SyntaxKind::$token,)* $(SyntaxKind::$kw,)* $(SyntaxKind::$punct,)*
                $(SyntaxKind::$joined,)* $(SyntaxKind::$node,)*
            ];
            /// The keywords of `r[lex.keywords]`, with their kinds.
            pub const KEYWORDS: &[(&str, SyntaxKind)] = &[$(($kw_text, SyntaxKind::$kw)),*];
            /// Every punctuation token the lexer produces, with its kind. `>=` and `>>` aren't
            /// among them: see [`SyntaxKind::JOINED`].
            pub const PUNCTUATION: &[(&str, SyntaxKind)] =
                &[$(($punct_text, SyntaxKind::$punct)),*];
            /// The tokens only the CST has: the tree builder joins two adjacent lexer tokens,
            /// a `>` and the token after it, into one of these when the parser asks.
            pub const JOINED: &[(&str, SyntaxKind)] =
                &[$(($joined_text, SyntaxKind::$joined)),*];
            /// Every lexer token kind that is neither a keyword nor punctuation, for tests that
            /// walk them all.
            #[cfg(test)]
            pub const TOKENS: &[SyntaxKind] = &[$(SyntaxKind::$token),*];
        }
    };
}

syntax_kinds! {
    tokens: [
        // Trivia (see the README for what the parser does with each).
        Whitespace, Newline, Comment, DocComment, BlockComment, Bom,
        /// Characters that start no token. Trivia to the parser.
        Error,
        Ident, Int, Float, Char, StringStart, StringText, StringEnd,
        /// A line ending inside a multi-line string.
        StringNewline,
        /// Spaces and tabs in a multi-line string that aren't part of its value.
        StringIndent,
        /// The `{` and `}` around an interpolated path in a string.
        InterpStart, InterpEnd,
        /// Reported as E0002, and read by the parser as a line break.
        Semicolon,
    ],
    keywords: [
        FnKw = "fn", LetKw = "let", VarKw = "var", TypeKw = "type", StructKw = "struct",
        EnumKw = "enum", InterfaceKw = "interface", ImplKw = "impl", MatchKw = "match",
        IsKw = "is", IfKw = "if", ElseKw = "else", ForKw = "for", InKw = "in",
        WhileKw = "while", ReturnKw = "return", BreakKw = "break", ContinueKw = "continue",
        ImportKw = "import", PubKw = "pub", UsesKw = "uses", RequiresKw = "requires",
        EnsuresKw = "ensures", TestKw = "test", ForallKw = "forall", WithKw = "with",
        ConcurrentKw = "concurrent", SpawnKw = "spawn", BackgroundKw = "background",
        ForeignKw = "foreign", TrueKw = "true", FalseKw = "false", SelfKw = "self",
        SelfTypeKw = "Self", ConstKw = "const", EffectKw = "effect", SelectKw = "select",
        AsKw = "as",
    ],
    punctuation: [
        Underscore = "_", Comma = ",", Colon = ":", Question = "?", At = "@", Tilde = "~",
        Dot = ".", DotDot = "..", DotDotEq = "..=",
        LParen = "(", RParen = ")", LBracket = "[", RBracket = "]", LBrace = "{", RBrace = "}",
        Plus = "+", PlusEq = "+=", Minus = "-", MinusEq = "-=", Arrow = "->",
        Star = "*", StarEq = "*=", Slash = "/", SlashEq = "/=", Percent = "%", PercentEq = "%=",
        Eq = "=", EqEq = "==", FatArrow = "=>", Bang = "!", BangEq = "!=",
        Lt = "<", LtEq = "<=", LtLt = "<<", Gt = ">",
        Amp = "&", AmpAmp = "&&", Pipe = "|", PipePipe = "||", Caret = "^",
    ],
    joined: [GtGt = ">>", GtEq = ">="],
    // Each node kind is named after the grammar production it holds, where there is one.
    // A layer of the parser adds the kinds it builds.
    nodes: [
        /// The whole file: the root, and the one node that may be empty.
        File,
        /// Tokens the parser skipped while recovering from an error.
        Skipped,
        FnDecl, ParamList, Param, RetType, TestDecl, ConstDecl, ImplDecl,
        Import,
        /// The names an import brings in, in braces.
        ImportNames,
        StructDecl,
        /// A struct's fields in braces.
        FieldList,
        /// A field being declared, in a struct or a variant.
        FieldDecl,
        NewtypeDecl, EnumDecl,
        /// An enum's variants in braces.
        VariantList,
        Variant,
        /// A variant's fields in parentheses.
        VariantFields,
        AliasDecl, Attribute, InterfaceDecl,
        /// `type` and a name, then `=` and a type in an `impl`.
        AssocType,
        EffectDecl, ForeignDecl,
        /// `type` and a C name, in a `foreign` block.
        ForeignType,
        /// The doc comments of a declaration, a field, a variant or a member.
        DocComments,
        /// A type path with its type arguments.
        TypePath, TypeArgs,
        /// A bound's type argument that names an associated type: `Item = Int`.
        NamedTypeArg,
        /// `()`, or a tuple of two or more types.
        TupleType,
        /// One type in parentheses: a function type, or an error.
        ParenType,
        FnType,
        /// `uses` and its list of effects.
        Effects,
        Generics, GenericParam,
        Block, LetStmt, VarStmt, WhileStmt,
        /// A place, `=` or a compound assignment, and an expression.
        AssignStmt,
        /// `if`, its condition and block, and `else` with an `IfExpr` or a block.
        IfExpr,
        /// A name being declared.
        Name,
        /// A name being used, `self` or `Self`.
        NameRef,
        Literal,
        /// An interpolation in a string: its braces and the tokens of its path.
        Interp,
        ListLit, Range,
        /// An operator between two expressions, other than a range's.
        Binary,
        /// `-`, `!` or `~`, and the expression it applies to.
        Prefix,
        /// An expression in parentheses.
        Paren,
        /// `()`, or a tuple of two or more expressions.
        Tuple,
        MapLit, MapEntry,
        /// An expression and an index in square brackets.
        Index,
        /// An expression and `?`.
        Try,
        /// An argument with a name: `email: e`, or `email:` alone.
        NamedArg,
        /// `..` and the base of a struct update, the first argument.
        UpdateArg,
        /// A call of anything: a function, a method (`Field` as callee), a struct or a variant.
        Call,
        ArgList,
        /// A field or a tuple position of the expression before the `.`.
        Field,
        /// An expression, `is` and a pattern.
        IsExpr,
        /// Two or more alternatives joined by `|`.
        OrPat,
        /// `_`.
        WildcardPat,
        /// A literal, as a `Literal`.
        LiteralPat,
        /// Two `Literal`s with `..` or `..=` between them.
        RangePat,
        /// A name the pattern binds, as a `Name`.
        BindPat,
        /// A path: a constant, or a variant with no fields.
        PathPat,
        /// A path and its fields in parentheses.
        VariantPat,
        /// A variant's field matched by name: `width: w`, or `width:` alone.
        FieldPat,
        /// Two or more patterns in parentheses.
        TuplePat,
        /// `match`, an expression and a `MatchArmList`.
        MatchExpr,
        /// A `match`'s arms in braces.
        MatchArmList,
        /// A pattern, a `MatchGuard` if present, `=>`, and an expression or a block.
        MatchArm,
        /// `if` and an expression, between an arm's pattern and its `=>`.
        MatchGuard,
        /// `for`, a pattern, `in`, an expression and a block.
        ForStmt,
        /// `fn`, its parameters, its return type and effects if present, and a block.
        Closure,
        /// `return` and an expression if present.
        ReturnExpr,
        BreakExpr, ContinueExpr,
        /// `requires` and an expression, or `ensures`, a name in parentheses if present, and
        /// an expression.
        Contract,
        /// `concurrent`, an `ArgList` if present, and a `SpawnList` or a `ForStmt`.
        ConcurrentExpr,
        /// A `concurrent`'s spawns in braces.
        SpawnList,
        /// `spawn`, `in` and a scope if present, and an expression.
        SpawnExpr,
        /// `background`, the scope's `Param` and an argument in parentheses, and a block.
        BackgroundExpr,
        /// `select` and a `SelectArmList`.
        SelectExpr,
        /// A `select`'s arms in braces.
        SelectArmList,
        /// A pattern, `=`, an expression, `=>`, and an expression or a block.
        SelectArm,
        /// `with`, its `Handler`s, and a block.
        WithExpr,
        /// An effect's `TypePath`, `=` and an expression.
        Handler,
    ],
}

impl SyntaxKind {
    /// The keyword spelled `text`, if it is one.
    pub fn keyword(text: &str) -> Option<SyntaxKind> {
        Self::KEYWORDS
            .iter()
            .find(|(kw, _)| *kw == text)
            .map(|&(_, kind)| kind)
    }

    pub fn is_keyword(self) -> bool {
        Self::KEYWORDS.iter().any(|&(_, kind)| kind == self)
    }

    /// True for the token kinds the parser never sees: whitespace, line endings, comments,
    /// block comments, the byte order mark, `Error` and `Semicolon`. A `DocComment` is trivia
    /// only where it documents nothing, which the parser decides, so it isn't one of these.
    pub fn is_trivia(self) -> bool {
        use SyntaxKind::*;
        matches!(
            self,
            Whitespace | Newline | Comment | BlockComment | Bom | Error | Semicolon
        )
    }

    /// True for the node kinds that take the comments directly above them: declarations,
    /// fields, enum variants, members, and match and select arms. The README's CST section has
    /// the rule.
    pub fn attaches_comments(self) -> bool {
        use SyntaxKind::*;
        matches!(
            self,
            FnDecl
                | TestDecl
                | ConstDecl
                | ImplDecl
                | Import
                | StructDecl
                | NewtypeDecl
                | EnumDecl
                | AliasDecl
                | InterfaceDecl
                | AssocType
                | EffectDecl
                | ForeignDecl
                | ForeignType
                | FieldDecl
                | Variant
                | MatchArm
                | SelectArm
        )
    }

    /// True for the kinds after which a line always continues, whatever their role
    /// (`r[lex.line-end]`, `r[lex.continue.more]`): a comma, an opening bracket, `=`, a compound
    /// assignment, `=>`, `->`, and every binary operator but `>` and `*`. `>` can close type
    /// arguments and `*` can be the effect set in `uses *`, so the parser decides those.
    pub fn continues_line(self) -> bool {
        use SyntaxKind::*;
        matches!(
            self,
            Comma
                | LParen
                | LBracket
                | LBrace
                | Eq
                | PlusEq
                | MinusEq
                | StarEq
                | SlashEq
                | PercentEq
                | FatArrow
                | Arrow
                | Plus
                | Minus
                | Slash
                | Percent
                | Amp
                | Pipe
                | Caret
                | LtLt
                | AmpAmp
                | PipePipe
                | EqEq
                | BangEq
                | Lt
                | LtEq
                | DotDot
                | DotDotEq
                | IsKw
        )
    }
}
