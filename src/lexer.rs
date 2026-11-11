use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Word(String),
    Number(String),
    Str(String),
    Backtick(String),
    Sym(String),
    Eof,
}

impl Tok {
    pub fn is_word(&self, w: &str) -> bool {
        matches!(self, Tok::Word(s) if s == w)
    }

    pub fn is_sym(&self, s: &str) -> bool {
        matches!(self, Tok::Sym(x) if x == s)
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Word(s) | Tok::Number(s) | Tok::Sym(s) | Tok::Str(s) | Tok::Backtick(s) => {
                write!(f, "{s}")
            }
            Tok::Eof => write!(f, "end of file"),
        }
    }
}

pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut col = 1usize;

    let peek = |i: usize| -> Option<char> { chars.get(i).copied() };

    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            i += 1;
            line += 1;
            col = 1;
            continue;
        }
        if c == '\r' {
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            col += 1;
            continue;
        }

        // Line comment. Long comments (--[[ ... ]]) are skipped as a whole.
        if c == '-' && peek(i + 1) == Some('-') {
            if peek(i + 2) == Some('[') && peek(i + 3) == Some('[') {
                i += 4;
                col += 4;
                while i < chars.len() {
                    if chars[i] == ']' && peek(i + 1) == Some(']') {
                        i += 2;
                        col += 2;
                        break;
                    }
                    if chars[i] == '\n' {
                        line += 1;
                        col = 1;
                    } else {
                        col += 1;
                    }
                    i += 1;
                }
            } else {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                    col += 1;
                }
            }
            continue;
        }

        let start_line = line;
        let start_col = col;

        // Strings.
        if c == '"' || c == '\'' {
            let quote = c;
            let mut s = String::new();
            s.push(c);
            i += 1;
            col += 1;
            while i < chars.len() {
                let ch = chars[i];
                s.push(ch);
                if ch == '\\' {
                    i += 1;
                    col += 1;
                    if i < chars.len() {
                        s.push(chars[i]);
                        if chars[i] == '\n' {
                            line += 1;
                            col = 1;
                        } else {
                            col += 1;
                        }
                        i += 1;
                    }
                    continue;
                }
                if ch == quote {
                    i += 1;
                    col += 1;
                    break;
                }
                if ch == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                i += 1;
            }
            out.push(Token {
                tok: Tok::Str(s),
                line: start_line,
                col: start_col,
            });
            continue;
        }

        // Backtick interpolated strings are kept as raw source. Luau uses the
        // same syntax, so expressions inside them need no lowering here.
        if c == '`' {
            let mut s = String::new();
            s.push(c);
            i += 1;
            col += 1;
            while i < chars.len() {
                let ch = chars[i];
                s.push(ch);
                if ch == '\\' {
                    i += 1;
                    col += 1;
                    if i < chars.len() {
                        s.push(chars[i]);
                        if chars[i] == '\n' {
                            line += 1;
                            col = 1;
                        } else {
                            col += 1;
                        }
                        i += 1;
                    }
                    continue;
                }
                if ch == '`' {
                    i += 1;
                    col += 1;
                    break;
                }
                if ch == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                i += 1;
            }
            out.push(Token {
                tok: Tok::Backtick(s),
                line: start_line,
                col: start_col,
            });
            continue;
        }

        // Numbers.
        if c.is_ascii_digit() {
            // A dot followed by digits after an expression is parsed as `.1`
            // field access by the parser, not as a number here.
            let mut s = String::new();
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                // Do not swallow `..` (concat).
                if chars[i] == '.' && peek(i + 1) == Some('.') {
                    break;
                }
                s.push(chars[i]);
                i += 1;
                col += 1;
            }
            out.push(Token {
                tok: Tok::Number(s),
                line: start_line,
                col: start_col,
            });
            continue;
        }

        // Words / identifiers / keywords. They are all Word tokens; the parser
        // treats keywords contextually, which also lets `class` appear as an
        // identifier in `class.fields(...)`.
        if c.is_alphabetic() || c == '_' {
            let mut s = String::new();
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                s.push(chars[i]);
                i += 1;
                col += 1;
            }
            out.push(Token {
                tok: Tok::Word(s),
                line: start_line,
                col: start_col,
            });
            continue;
        }

        // Multi-character symbols.
        let rest: String = chars[i..(i + 4).min(chars.len())].iter().collect();
        let mut matched = None;
        for sym in ["...", "..=", "..", "==", "~=", "<=", ">=", "+=", "-=", "*=", "/=", "%=", "^=", "::", "->", "<-", "<+", "//"] {
            if rest.starts_with(sym) {
                matched = Some(sym);
                break;
            }
        }
        if let Some(sym) = matched {
            out.push(Token {
                tok: Tok::Sym(sym.to_string()),
                line: start_line,
                col: start_col,
            });
            i += sym.chars().count();
            col += sym.chars().count();
            continue;
        }

        // Single-character symbol.
        out.push(Token {
            tok: Tok::Sym(c.to_string()),
            line: start_line,
            col: start_col,
        });
        i += 1;
        col += 1;
    }

    out.push(Token {
        tok: Tok::Eof,
        line,
        col,
    });
    Ok(out)
}
