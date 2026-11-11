//! `SyntaxKind`: every kind of token, and later every kind of CST node.

macro_rules! syntax_kinds {
    (
        tokens: [$($(#[$meta:meta])* $token:ident),* $(,)?],
        keywords: [$($kw:ident = $kw_text:literal),* $(,)?],
        punctuation: [$($punct:ident = $punct_text:literal),* $(,)?] $(,)?
    ) => {
        /// The kind of a token. The parser will add its node kinds to the same enum.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u16)]
        pub enum SyntaxKind {
            $($(#[$meta])* $token,)*
            $($kw,)*
            $($punct,)*
        }

        impl SyntaxKind {
            /// The keywords of `r[lex.keywords]`, with their kinds.
            pub const KEYWORDS: &[(&str, SyntaxKind)] = &[$(($kw_text, SyntaxKind::$kw)),*];
            /// Every punctuation token, with its kind. `>=` and `>>` aren't tokens: the parser
            /// joins two adjacent tokens to form them.
            pub const PUNCTUATION: &[(&str, SyntaxKind)] =
                &[$(($punct_text, SyntaxKind::$punct)),*];
        }
    };
}

syntax_kinds! {
    tokens: [
        // Trivia (see the README for what the parser does with each).
        Whitespace, Newline, Comment, DocComment, BlockComment, Bom,
        /// Characters that start no token. Trivia to the parser.
        Error,
        Ident, Int, Char, StringStart, StringText, StringEnd,
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
}

impl SyntaxKind {
    /// The keyword spelled `text`, if it is one.
    pub fn keyword(text: &str) -> Option<SyntaxKind> {
        Self::KEYWORDS
            .iter()
            .find(|(kw, _)| *kw == text)
            .map(|&(_, kind)| kind)
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
