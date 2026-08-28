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
    // atoms
    Ident(String),
    Number(f64),
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
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    Ne,
    Eof,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Spanned {
    pub tok: Token,
    pub line: usize,
    pub col: usize,
}

pub fn tokenize(src: &str) -> Result<Vec<Spanned>, ParseError> {
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

        // number: digits with an optional single fractional part
        if c.is_ascii_digit() {
            let mut s = String::new();
            while i < chars.len() && chars[i].is_ascii_digit() {
                s.push(chars[i]);
                i += 1;
                col += 1;
            }
            if i < chars.len() && chars[i] == '.' {
                s.push('.');
                i += 1;
                col += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    s.push(chars[i]);
                    i += 1;
                    col += 1;
                }
            }
            let n: f64 = s
                .parse()
                .map_err(|_| ParseError::new(format!("invalid number '{s}'"), sl, sc))?;
            out.push(Spanned {
                tok: Token::Number(n),
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
            let tok = match s.as_str() {
                "export" => Token::Export,
                "fn" => Token::Fn,
                "if" => Token::If,
                "return" => Token::Return,
                "true" => Token::True,
                "false" => Token::False,
                "error" => Token::Error,
                "fail" => Token::Fail,
                _ => Token::Ident(s),
            };
            out.push(Spanned {
                tok,
                line: sl,
                col: sc,
            });
            continue;
        }

        return Err(ParseError::new(
            format!("unexpected character '{c}'"),
            sl,
            sc,
        ));
    }

    out.push(Spanned {
        tok: Token::Eof,
        line,
        col,
    });
    Ok(out)
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
    fn lexes_keywords_idents_numbers_and_skips_comments() {
        assert_eq!(
            toks("// c\nexport fn x 0.9 12 true"),
            vec![
                Token::Export,
                Token::Fn,
                Token::Ident("x".into()),
                Token::Number(0.9),
                Token::Number(12.0),
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
}
