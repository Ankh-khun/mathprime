use crate::error::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Num(f64),
    /// Imaginair getal: `2i`, `0.5j`
    Imag(f64),
    Ident(String),
    /// Tekst: `"abc"`
    Str(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    /// `..` (bereik)
    DotDot,
    /// `:=` (definitie)
    Assign,
    /// `=` (resultaat tonen)
    Eq,
    /// `==` (vergelijken)
    EqEq,
    /// `!=` of `≠`
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// `and` of `∧`
    And,
    /// `or` of `∨`
    Or,
    /// `not` of `¬`
    Not,
    /// `→` of `->` (symbolisch uitrekenen)
    Arrow,
    /// `!` (faculteit)
    Bang,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    /// Byte-offsets in de brontekst (`start..end`).
    pub start: usize,
    pub end: usize,
}

fn lex_err(pos: usize, msg: impl Into<String>) -> Error {
    Error::Lex {
        pos,
        msg: msg.into(),
    }
}

fn ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '°'
}

fn ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '°'
}

/// Splits een regel in tokens. Het laatste token is altijd `Eof`.
///
/// Namen mogen unicode-letters bevatten (σ, τ, Δ) en letterlijke subscripts met een punt
/// (`M.max`, `F.x`), net als in Mathcad. `°C` en `Δ°C` zijn ook namen.
pub fn lex(src: &str) -> Result<Vec<Token>, Error> {
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let byte_at = |idx: usize| -> usize {
        if idx < chars.len() {
            chars[idx].0
        } else {
            src.len()
        }
    };
    let ch = |idx: usize| chars.get(idx).map(|&(_, c)| c);
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '.' && ch(i + 1) == Some('.') {
            out.push(Token {
                tok: Tok::DotDot,
                start,
                end: byte_at(i + 2),
            });
            i += 2;
            continue;
        }
        let next_is_digit = ch(i + 1).map_or(false, |d| d.is_ascii_digit());
        if c.is_ascii_digit() || (c == '.' && next_is_digit) {
            let mut j = i;
            while j < chars.len() && chars[j].1.is_ascii_digit() {
                j += 1;
            }
            // een decimale punt, maar niet het begin van `..` (zoals in `1..10`)
            if ch(j) == Some('.') && ch(j + 1) != Some('.') {
                j += 1;
                while j < chars.len() && chars[j].1.is_ascii_digit() {
                    j += 1;
                }
            }
            // exponent: 1e3, 2.5E-4 (maar niet `3 eV` of `2e`)
            if matches!(ch(j), Some('e') | Some('E')) {
                let mut k = j + 1;
                if matches!(ch(k), Some('+') | Some('-')) {
                    k += 1;
                }
                if ch(k).map_or(false, |d| d.is_ascii_digit()) {
                    while ch(k).map_or(false, |d| d.is_ascii_digit()) {
                        k += 1;
                    }
                    j = k;
                }
            }
            let end = byte_at(j);
            let text = &src[start..end];
            let v: f64 = text
                .parse()
                .map_err(|_| lex_err(start, format!("ongeldig getal '{text}'")))?;
            // `2i` of `2j` is imaginair, maar `2in` (inch) of `2ij` niet
            if matches!(ch(j), Some('i') | Some('j')) && !ch(j + 1).map_or(false, ident_continue) {
                out.push(Token {
                    tok: Tok::Imag(v),
                    start,
                    end: byte_at(j + 1),
                });
                i = j + 1;
                continue;
            }
            out.push(Token {
                tok: Tok::Num(v),
                start,
                end,
            });
            i = j;
            continue;
        }
        // tekst tussen aanhalingstekens (`"abc"`); `""` binnenin is één aanhalingsteken
        if c == '"' {
            let mut text = String::new();
            let mut j = i + 1;
            loop {
                match ch(j) {
                    None => return Err(lex_err(start, "de tekst wordt niet afgesloten met '\"'")),
                    Some('"') if ch(j + 1) == Some('"') => {
                        text.push('"');
                        j += 2;
                    }
                    Some('"') => break,
                    Some(d) => {
                        text.push(d);
                        j += 1;
                    }
                }
            }
            out.push(Token { tok: Tok::Str(text), start, end: byte_at(j + 1) });
            i = j + 1;
            continue;
        }
        // losse wiskundesymbolen die als naam werken
        if matches!(c, '∞' | '∑' | '∏' | '∫' | '∈') {
            let end = byte_at(i + 1);
            out.push(Token {
                tok: Tok::Ident(c.to_string()),
                start,
                end,
            });
            i += 1;
            continue;
        }
        if ident_start(c) {
            let mut j = i + 1;
            while j < chars.len() {
                let d = chars[j].1;
                if ident_continue(d) {
                    j += 1;
                } else if d == '.' && ch(j + 1).map_or(false, |e| e.is_alphanumeric()) {
                    j += 1;
                } else {
                    break;
                }
            }
            // accenten voor afgeleiden in odesolve: y', y''
            while ch(j) == Some('\'') {
                j += 1;
            }
            let end = byte_at(j);
            let word = &src[start..end];
            let tok = match word {
                "and" => Tok::And,
                "or" => Tok::Or,
                "not" => Tok::Not,
                _ => Tok::Ident(word.to_string()),
            };
            out.push(Token { tok, start, end });
            i = j;
            continue;
        }
        let next = ch(i + 1);
        let (tok, len) = match c {
            '+' => (Tok::Plus, 1),
            '-' if next == Some('>') => (Tok::Arrow, 2),
            '→' => (Tok::Arrow, 1),
            '-' | '−' => (Tok::Minus, 1),
            '*' | '×' | '·' => (Tok::Star, 1),
            '/' | '÷' => (Tok::Slash, 1),
            '^' => (Tok::Caret, 1),
            '(' => (Tok::LParen, 1),
            ')' => (Tok::RParen, 1),
            '[' => (Tok::LBracket, 1),
            ']' => (Tok::RBracket, 1),
            ',' => (Tok::Comma, 1),
            ';' => (Tok::Semicolon, 1),
            '…' => (Tok::DotDot, 1),
            '=' if next == Some('=') => (Tok::EqEq, 2),
            '=' => (Tok::Eq, 1),
            '<' if next == Some('=') => (Tok::Le, 2),
            '<' => (Tok::Lt, 1),
            '>' if next == Some('=') => (Tok::Ge, 2),
            '>' => (Tok::Gt, 1),
            '≤' => (Tok::Le, 1),
            '≥' => (Tok::Ge, 1),
            '≠' => (Tok::Ne, 1),
            '!' if next == Some('=') => (Tok::Ne, 2),
            '!' => (Tok::Bang, 1),
            '∧' => (Tok::And, 1),
            '∨' => (Tok::Or, 1),
            '¬' => (Tok::Not, 1),
            ':' if next == Some('=') => (Tok::Assign, 2),
            // lokale toekenning in een programmablok, zoals in Mathcad
            '←' => (Tok::Assign, 1),
            ':' => return Err(lex_err(start, "':' hoort alleen in ':=' (definitie)")),
            other => return Err(lex_err(start, format!("onverwacht teken '{other}'"))),
        };
        out.push(Token {
            tok,
            start,
            end: byte_at(i + len),
        });
        i += len;
    }
    out.push(Token {
        tok: Tok::Eof,
        start: src.len(),
        end: src.len(),
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Tok> {
        lex(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn getallen_en_exponent() {
        assert_eq!(kinds("2.5e-3")[0], Tok::Num(0.0025));
        assert_eq!(kinds(".5")[0], Tok::Num(0.5));
        // `3 eV`-achtig: de e hoort bij de naam, niet bij het getal
        assert_eq!(kinds("3eV")[..2], [Tok::Num(3.0), Tok::Ident("eV".into())]);
    }

    #[test]
    fn namen_met_subscript_en_unicode() {
        assert_eq!(kinds("M.max")[0], Tok::Ident("M.max".into()));
        assert_eq!(kinds("σ_vm")[0], Tok::Ident("σ_vm".into()));
        // een punt gevolgd door een getal na een naam is geen decimaal: x.1 is een subscript
        assert_eq!(kinds("x.1")[0], Tok::Ident("x.1".into()));
        assert_eq!(kinds("Δ°C")[0], Tok::Ident("Δ°C".into()));
        assert_eq!(kinds("20 °C")[1], Tok::Ident("°C".into()));
    }

    #[test]
    fn operatoren_en_assign() {
        assert_eq!(
            kinds("a := b^2 − c×d")[..],
            [
                Tok::Ident("a".into()),
                Tok::Assign,
                Tok::Ident("b".into()),
                Tok::Caret,
                Tok::Num(2.0),
                Tok::Minus,
                Tok::Ident("c".into()),
                Tok::Star,
                Tok::Ident("d".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn bereik_en_imaginair() {
        assert_eq!(
            kinds("1..10")[..3],
            [Tok::Num(1.0), Tok::DotDot, Tok::Num(10.0)]
        );
        assert_eq!(
            kinds("0, 0.5 .. 3")[..5],
            [Tok::Num(0.0), Tok::Comma, Tok::Num(0.5), Tok::DotDot, Tok::Num(3.0)]
        );
        assert_eq!(kinds("2i")[0], Tok::Imag(2.0));
        assert_eq!(kinds("1.5j")[0], Tok::Imag(1.5));
        // inch blijft inch
        assert_eq!(kinds("2in")[..2], [Tok::Num(2.0), Tok::Ident("in".into())]);
    }

    #[test]
    fn vergelijken_en_logica() {
        assert_eq!(
            kinds("a <= b == c != d ≥ e")
                .into_iter()
                .filter(|t| !matches!(t, Tok::Ident(_) | Tok::Eof))
                .collect::<Vec<_>>(),
            vec![Tok::Le, Tok::EqEq, Tok::Ne, Tok::Ge]
        );
        assert_eq!(kinds("not a and b or c")[0], Tok::Not);
        assert_eq!(kinds("n!")[1], Tok::Bang);
    }

    #[test]
    fn fouten() {
        assert!(lex("a : b").is_err());
        assert!(lex("a # b").is_err());
    }
}
