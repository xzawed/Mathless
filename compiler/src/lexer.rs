//! Lexer for the Mathless MVP subset. Tracks line/col so the parser can report
//! positions. Skips whitespace and `// line comments`.

use crate::error::ParseError;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    // keywords
    Export,
    Fn,
    If,
    Return,
    True,
    False,
    Error,
    Fail,
    /// `try` — the call-site marker for a fallible callee (SPEC-fallible-calls DP-F1).
    Try,
    Let,
    Mut,
    /// `out` — marks a caller-allocates out-parameter (SPEC-out-params).
    Out,
    While,
    As,
    // atoms
    Ident(String),
    /// A float literal (has a decimal point), e.g. `0.9`.
    Number(f64),
    /// An integer literal (no decimal point), e.g. `12`.
    Int(i64),
    /// `"…"` — an ASCII, unescaped string literal (SPEC-string-input DP-S4).
    Str(String),
    // punctuation
    Arrow,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Assign,
    Bang,
    // operators
    Plus,
    Minus,
    Star,
    Slash,
    /// `%` — remainder (SPEC-i32-division). i32 only; sits with `*` and `/`.
    Percent,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    Ne,
    AndAnd,
    OrOr,
    Eof,
}

impl Token {
    /// The surface spelling, if this token is a keyword. Lets the parser say "`mut` is a
    /// keyword" instead of dumping the `Debug` variant name at the user.
    pub fn keyword_text(&self) -> Option<&'static str> {
        match self {
            Token::Export => Some("export"),
            Token::Fn => Some("fn"),
            Token::If => Some("if"),
            Token::Return => Some("return"),
            Token::True => Some("true"),
            Token::False => Some("false"),
            Token::Error => Some("error"),
            Token::Fail => Some("fail"),
            Token::Try => Some("try"),
            Token::Let => Some("let"),
            Token::Mut => Some("mut"),
            Token::Out => Some("out"),
            Token::While => Some("while"),
            Token::As => Some("as"),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Spanned {
    pub tok: Token,
    pub line: usize,
    pub col: usize,
}

pub fn tokenize(src: &str) -> Result<Vec<Spanned>, ParseError> {
    match tokenize_partial(src) {
        (tokens, None) => Ok(tokens),
        (_, Some(e)) => Err(e),
    }
}

/// Everything lexed before the first bad character, plus that error if there was one.
///
/// The whole file is lexed before the parser sees a single token, so a bad CHARACTER late in
/// the source hides a real error EARLY in it: `struct P { x: i32 }` on line 1 with a `p.x` on
/// line 2 reported the `.`, because the struct is only wrong to the *parser* and the parser
/// never ran (STATUS §9-2, measured). Returning the prefix lets the caller parse it anyway and
/// keep whichever error comes first in the file.
///
/// The prefix always ends with `Eof`, so the parser cannot run off the end of it.
pub fn tokenize_partial(src: &str) -> (Vec<Spanned>, Option<ParseError>) {
    /// Stop lexing here: close the prefix with `Eof` so it is parseable on its own, and hand
    /// back the error beside it.
    fn stop(mut out: Vec<Spanned>, e: ParseError) -> (Vec<Spanned>, Option<ParseError>) {
        out.push(Spanned {
            tok: Token::Eof,
            line: e.line,
            col: e.col,
        });
        (out, Some(e))
    }

    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut col = 1usize;
    let mut out: Vec<Spanned> = Vec::new();

    while i < chars.len() {
        let c = chars[i];

        // newlines / whitespace
        if c == '\n' {
            i += 1;
            line += 1;
            col = 1;
            continue;
        }
        if c == ' ' || c == '\t' || c == '\r' {
            i += 1;
            col += 1;
            continue;
        }
        // line comment
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
                col += 1;
            }
            continue;
        }

        let (sl, sc) = (line, col);

        // two-char operators (check before single-char)
        if c == '-' && i + 1 < chars.len() && chars[i + 1] == '>' {
            out.push(Spanned {
                tok: Token::Arrow,
                line: sl,
                col: sc,
            });
            i += 2;
            col += 2;
            continue;
        }
        // `&&` / `||`. The single-character forms are deliberately NOT tokens: bitwise
        // operators are out of scope, so `&` and `|` fall through to the error path below,
        // which names the operator the writer almost certainly meant.
        if (c == '&' || c == '|') && i + 1 < chars.len() && chars[i + 1] == c {
            out.push(Spanned {
                tok: if c == '&' { Token::AndAnd } else { Token::OrOr },
                line: sl,
                col: sc,
            });
            i += 2;
            col += 2;
            continue;
        }
        if i + 1 < chars.len() && chars[i + 1] == '=' {
            let two = match c {
                '<' => Some(Token::Le),
                '>' => Some(Token::Ge),
                '=' => Some(Token::EqEq),
                '!' => Some(Token::Ne),
                _ => None,
            };
            if let Some(tok) = two {
                out.push(Spanned {
                    tok,
                    line: sl,
                    col: sc,
                });
                i += 2;
                col += 2;
                continue;
            }
        }

        // single-char punctuation / operators
        let single = match c {
            '(' => Some(Token::LParen),
            ')' => Some(Token::RParen),
            '{' => Some(Token::LBrace),
            '}' => Some(Token::RBrace),
            ',' => Some(Token::Comma),
            ':' => Some(Token::Colon),
            '=' => Some(Token::Assign),
            '!' => Some(Token::Bang),
            '+' => Some(Token::Plus),
            '-' => Some(Token::Minus),
            '*' => Some(Token::Star),
            '/' => Some(Token::Slash),
            '%' => Some(Token::Percent),
            '<' => Some(Token::Lt),
            '>' => Some(Token::Gt),
            _ => None,
        };
        if let Some(tok) = single {
            out.push(Spanned {
                tok,
                line: sl,
                col: sc,
            });
            i += 1;
            col += 1;
            continue;
        }

        // string literal: `"…"`, plain ASCII, no escapes (SPEC-string-input DP-S4).
        //
        // No escapes means `"` cannot appear inside one, which is fine for the classification
        // codes this exists for. `\` is rejected loudly rather than passed through, so a
        // source that expects `\n` to mean a newline fails instead of silently comparing a
        // backslash. ASCII-only keeps STATUS section 6's existing rule (non-ASCII in a
        // generated artifact trips MSVC C4819 under `/WX`) narrow rather than widening it.
        if c == '"' {
            let (line0, col0) = (line, col);
            i += 1;
            col += 1;
            let mut s = String::new();
            loop {
                let Some(&ch) = chars.get(i) else {
                    return stop(
                        out,
                        ParseError::new(
                            "unterminated string literal — a string must close on the same line",
                            line0,
                            col0,
                        ),
                    );
                };
                match ch {
                    '"' => {
                        i += 1;
                        col += 1;
                        break;
                    }
                    '\n' => {
                        return stop(
                            out,
                            ParseError::new(
                                "unterminated string literal — a string must close on the \
                                 same line",
                                line0,
                                col0,
                            ),
                        )
                    }
                    '\\' => {
                        return stop(
                            out,
                            ParseError::new(
                                "escapes are not supported in a string literal yet — `\\` has \
                                 no meaning here, so it would silently be a backslash",
                                line,
                                col,
                            ),
                        )
                    }
                    c if !c.is_ascii() => {
                        return stop(
                            out,
                            ParseError::new(
                                format!(
                                    "non-ASCII character '{c}' in a string literal — generated \
                                     artifacts stay ASCII (non-ASCII trips MSVC C4819 under \
                                     `/WX`)"
                                ),
                                line,
                                col,
                            ),
                        )
                    }
                    c => {
                        s.push(c);
                        i += 1;
                        col += 1;
                    }
                }
            }
            out.push(Spanned {
                tok: Token::Str(s),
                line: line0,
                col: col0,
            });
            continue;
        }

        // number: digits with an optional single fractional part. A decimal point makes it a
        // float (`f64`); without one it is an integer literal (`i32`, carried as `i64` here).
        if c.is_ascii_digit() {
            let mut s = String::new();
            while i < chars.len() && chars[i].is_ascii_digit() {
                s.push(chars[i]);
                i += 1;
                col += 1;
            }
            let mut has_dot = false;
            if i < chars.len() && chars[i] == '.' {
                has_dot = true;
                s.push('.');
                i += 1;
                col += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    s.push(chars[i]);
                    i += 1;
                    col += 1;
                }
            }
            let tok = if has_dot {
                let Ok(n) = s.parse::<f64>() else {
                    return stop(
                        out,
                        ParseError::new(format!("invalid number '{s}'"), sl, sc),
                    );
                };
                // `f64::from_str` yields `inf` on overflow (never `Err`); reject it so codegen
                // never emits an invalid `inff64`. (NaN is impossible from a digit string;
                // negatives are a separate `Minus` token — so `is_finite` catches only overflow.)
                if !n.is_finite() {
                    return stop(
                        out,
                        ParseError::new(
                            "number literal is out of range for f64 (overflows to infinity)",
                            sl,
                            sc,
                        ),
                    );
                }
                Token::Number(n)
            } else {
                let Ok(n) = s.parse::<i64>() else {
                    return stop(
                        out,
                        ParseError::new(
                            format!("integer literal '{s}' is out of range for i64"),
                            sl,
                            sc,
                        ),
                    );
                };
                Token::Int(n)
            };
            out.push(Spanned {
                tok,
                line: sl,
                col: sc,
            });
            continue;
        }

        // identifier / keyword
        if c.is_alphabetic() || c == '_' {
            let mut s = String::new();
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                s.push(chars[i]);
                i += 1;
                col += 1;
            }
            // Identifiers are emitted RAW into every backend — the `.h`, the `.pas`, and the
            // `#[no_mangle]` Rust — so they must be ASCII for the same reason a string literal
            // must be, ninety lines above: a generated artifact that is not ASCII trips MSVC
            // C4819 under `/WX`. `emit.rs` already refuses a non-ASCII MODULE name; identifiers
            // were the hole between the two, and they are the ones that reach the header.
            //
            // Measured before this check: `export fn café(…)` compiled and then failed inside
            // GENERATED Rust with `#[no_mangle] requires ASCII identifier`, and
            // `export fn f(가격: f64)` built successfully and wrote a `.h` MSVC read as valid
            // under CP949 and rejected under CP1252 — a header whose meaning depends on the
            // reader's code page is worse than one that simply fails.
            if let Some(bad) = s.chars().find(|c| !c.is_ascii()) {
                return stop(
                    out,
                    ParseError::new(
                        format!(
                            "'{bad}' is not allowed in a name — identifiers must be ASCII \
                             letters, digits and `_`, because they are written unchanged into \
                             the generated C header and Delphi unit"
                        ),
                        sl,
                        sc,
                    ),
                );
            }
            let tok = match s.as_str() {
                "export" => Token::Export,
                "fn" => Token::Fn,
                "if" => Token::If,
                "return" => Token::Return,
                "true" => Token::True,
                "false" => Token::False,
                "error" => Token::Error,
                "fail" => Token::Fail,
                "try" => Token::Try,
                "let" => Token::Let,
                "mut" => Token::Mut,
                "out" => Token::Out,
                "while" => Token::While,
                "as" => Token::As,
                _ => Token::Ident(s),
            };
            out.push(Spanned {
                tok,
                line: sl,
                col: sc,
            });
            continue;
        }

        // `a & b` is the most likely typo now that `&&` exists, and "unexpected character"
        // is a poor answer to it. Mathless has no bitwise operators (SPEC-logical-ops DP-B1).
        //
        // The same reasoning covers the LANGUAGE gaps, and they were measured to need it:
        // writing twelve ordinary business rules in today's surface, all three that failed
        // failed on a character. `lines: f64[]` said "unexpected character '['" — a user is
        // told their bracket is wrong when what is missing is arrays. Naming the feature
        // costs one arm each and turns a puzzle into a known limit.
        let msg = match c {
            '&' => "`&` is not an operator in Mathless — did you mean `&&`?".to_string(),
            '|' => "`|` is not an operator in Mathless — did you mean `||`?".to_string(),
            '[' | ']' => {
                "arrays are not in Mathless yet — `[` is not valid in a type or an expression"
                    .to_string()
            }
            // A `.` inside a number is consumed by the literal path above, so one arriving
            // here is a field access (`c.tier`) or a stray dot — EXCEPT after a range, where
            // the number path has already eaten the first dot of `0..3` as part of `0.` and
            // left the second one here. Without this arm `for i in 0..3` was reported as
            // "field access", which is a confident wrong answer; the whole point of this
            // change is not to give those.
            // Either side, because the two spellings arrive differently: in `0..3` the number
            // path already ate `0.` and leaves a dot whose PREVIOUS char is a dot, while in
            // `a..b` this is the first dot and its NEXT one is the giveaway (Grok verify).
            '.' if (i > 0 && chars[i - 1] == '.')
                || chars.get(i + 1).is_some_and(|n| *n == '.') =>
            {
                "ranges are not in Mathless yet — `..` is not part of the language".to_string()
            }
            '.' => "field access is not in Mathless yet — `.` is only part of a number literal"
                .to_string(),
            '?' => {
                "`?` is not in Mathless — there is no option or null-safety type yet".to_string()
            }
            _ => format!("unexpected character '{c}'"),
        };
        out.push(Spanned {
            tok: Token::Eof,
            line: sl,
            col: sc,
        });
        return (out, Some(ParseError::new(msg, sl, sc)));
    }

    out.push(Spanned {
        tok: Token::Eof,
        line,
        col,
    });
    (out, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Token> {
        tokenize(src).unwrap().into_iter().map(|s| s.tok).collect()
    }

    #[test]
    fn lexes_arrow_and_ops_not_greedily() {
        assert_eq!(
            toks("-> - >= > <= == != * /"),
            vec![
                Token::Arrow,
                Token::Minus,
                Token::Ge,
                Token::Gt,
                Token::Le,
                Token::EqEq,
                Token::Ne,
                Token::Star,
                Token::Slash,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn lexes_mut_as_a_keyword_not_an_identifier() {
        // WM1: `mut` modifies `let`; it must not fall through to `Token::Ident`.
        assert_eq!(
            toks("let mut x"),
            vec![
                Token::Let,
                Token::Mut,
                Token::Ident("x".to_string()),
                Token::Eof,
            ]
        );
        // Still a plain identifier when it only starts with the same letters.
        assert_eq!(
            toks("mutable"),
            vec![Token::Ident("mutable".to_string()), Token::Eof]
        );
    }

    #[test]
    fn lexes_while_as_a_keyword_not_an_identifier() {
        // WW1: `while` starts a loop statement; a name that merely begins with it does not.
        assert_eq!(
            toks("while b"),
            vec![Token::While, Token::Ident("b".to_string()), Token::Eof]
        );
        assert_eq!(
            toks("whilex"),
            vec![Token::Ident("whilex".to_string()), Token::Eof]
        );
    }

    #[test]
    fn lexes_keywords_idents_numbers_and_skips_comments() {
        assert_eq!(
            toks("// c\nexport fn x 0.9 12 true"),
            vec![
                Token::Export,
                Token::Fn,
                Token::Ident("x".into()),
                Token::Number(0.9),
                Token::Int(12),
                Token::True,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn rejects_unexpected_char_with_position() {
        let e = tokenize("fn @").unwrap_err();
        assert!(e.message.contains('@'));
        assert_eq!((e.line, e.col), (1, 4));
    }

    #[test]
    fn rejects_a_float_literal_that_overflows_f64() {
        // A huge DECIMAL literal overflows f64 to `inf`; that must be a lex error, not a silent
        // `inf` that codegen would emit as invalid Rust (`inff64`). (A no-dot literal would be
        // an integer, so the `.0` is what routes this to the f64 path.)
        let e = tokenize(&format!("{}.0", "9".repeat(400))).unwrap_err();
        assert!(
            e.message.to_lowercase().contains("f64"),
            "message was: {}",
            e.message
        );
    }

    #[test]
    fn rejects_an_integer_literal_that_overflows_i64() {
        // A no-dot literal is an integer; one that exceeds i64 is a lex error.
        let e = tokenize(&"9".repeat(40)).unwrap_err();
        assert!(
            e.message.to_lowercase().contains("range"),
            "message was: {}",
            e.message
        );
    }
}
