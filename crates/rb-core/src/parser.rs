use crate::ast::{BStmt, BinOp, BindOp, CmpOp, Expr, Stmt};
use crate::error::Error;
use crate::lexer::{lex, Tok, Token};

// Bindingskracht (links, rechts). Hogere getallen binden sterker.
// Juxtapositie (`2 pi r`, `5 kN`) heeft dezelfde kracht als `*` en `/` en is links-associatief:
// `1/2 x` is dus (1/2)·x. Gebruik haakjes voor een noemer: `1/(2 x)`.
const BP_OR: (u8, u8) = (2, 3);
const BP_AND: (u8, u8) = (4, 5);
const BP_NOT: u8 = 6; // `not a < b` is not (a < b), maar `not a and b` is (not a) and b
const BP_CMP: (u8, u8) = (7, 8);
const BP_ADD: (u8, u8) = (10, 11);
const BP_MUL: (u8, u8) = (20, 21);
// naast elkaar (`50 Ω`, `2 π`) bindt sterker dan * en /: `230 V / 50 Ω` is (230 V) / (50 Ω)
const BP_JUX: (u8, u8) = (22, 23);
const BP_POW: (u8, u8) = (30, 29); // rechts-associatief
const BP_NEG: u8 = 25; // `-x^2` is -(x^2), maar `-a*b` is (-a)*b
const BP_POSTFIX: u8 = 40; // `n!`, `v[i]`

struct Parser<'a> {
    toks: &'a [Token],
    src: &'a str,
    pos: usize,
}

fn cmp_op(t: &Tok) -> Option<CmpOp> {
    Some(match t {
        Tok::Lt => CmpOp::Lt,
        Tok::Le => CmpOp::Le,
        Tok::Gt => CmpOp::Gt,
        Tok::Ge => CmpOp::Ge,
        Tok::EqEq => CmpOp::Eq,
        Tok::Ne => CmpOp::Ne,
        _ => return None,
    })
}

/// Namen die een variabele binden, met het toegestane aantal argumenten en de schrijfwijze.
fn binder(name: &str) -> Option<(BindOp, &'static [usize], &'static str)> {
    Some(match name {
        "sum" | "Σ" | "∑" => (BindOp::Sum, &[4], "sum(uitdrukking, i, van, tot)"),
        "prod" | "∏" => (BindOp::Prod, &[4], "prod(uitdrukking, i, van, tot)"),
        "integral" | "∫" => (BindOp::Integral, &[4], "integral(uitdrukking, x, a, b)"),
        "deriv" => (BindOp::Deriv, &[3, 4], "deriv(uitdrukking, x, punt) of deriv(uitdrukking, x, punt, orde)"),
        "root" => (BindOp::Root, &[2, 4], "root(uitdrukking, x) of root(uitdrukking, x, a, b)"),
        _ => return None,
    })
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Token {
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }

    fn advance(&mut self) -> &Token {
        let i = self.pos.min(self.toks.len() - 1);
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        &self.toks[i]
    }

    fn text(&self, t: &Token) -> String {
        if t.tok == Tok::Eof {
            "einde van de invoer".to_string()
        } else {
            format!("'{}'", &self.src[t.start..t.end])
        }
    }

    fn unexpected(&self) -> Error {
        let t = self.peek();
        Error::parse(t.start, format!("onverwacht {}", self.text(t)))
    }

    fn expect_eof(&self) -> Result<(), Error> {
        if self.peek().tok == Tok::Eof {
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, Error> {
        let mut lhs = self.parse_prefix()?;
        loop {
            let tok = self.peek().tok.clone();
            match tok {
                Tok::Bang => {
                    if BP_POSTFIX < min_bp {
                        break;
                    }
                    self.advance();
                    lhs = Expr::Call("factorial".to_string(), vec![lhs]);
                    continue;
                }
                Tok::LBracket => {
                    if BP_POSTFIX < min_bp {
                        break;
                    }
                    let open = self.advance().start;
                    let idx = self.parse_list(Tok::RBracket, open, "index")?;
                    if idx.is_empty() || idx.len() > 2 {
                        return Err(Error::parse(open, "een index heeft 1 getal (v[i]) of 2 getallen (M[i, j])"));
                    }
                    lhs = Expr::Index(Box::new(lhs), idx);
                    continue;
                }
                Tok::And | Tok::Or => {
                    let (lbp, rbp) = if tok == Tok::And { BP_AND } else { BP_OR };
                    if lbp < min_bp {
                        break;
                    }
                    self.advance();
                    let rhs = self.parse_expr(rbp)?;
                    lhs = if tok == Tok::And {
                        Expr::And(Box::new(lhs), Box::new(rhs))
                    } else {
                        Expr::Or(Box::new(lhs), Box::new(rhs))
                    };
                    continue;
                }
                ref t if cmp_op(t).is_some() => {
                    if BP_CMP.0 < min_bp {
                        break;
                    }
                    let mut ops = Vec::new();
                    let mut operands = vec![lhs];
                    while let Some(op) = cmp_op(&self.peek().tok) {
                        self.advance();
                        ops.push(op);
                        operands.push(self.parse_expr(BP_CMP.1)?);
                    }
                    lhs = Expr::Cmp(ops, operands);
                    continue;
                }
                _ => {}
            }
            let (op, (lbp, rbp), juxtaposition) = match tok {
                Tok::Plus => (BinOp::Add, BP_ADD, false),
                Tok::Minus => (BinOp::Sub, BP_ADD, false),
                Tok::Star => (BinOp::Mul, BP_MUL, false),
                Tok::Slash => (BinOp::Div, BP_MUL, false),
                Tok::Caret => (BinOp::Pow, BP_POW, false),
                // `5 kN`, `2 pi`, `2(x+1)`, `(a)(b)`: impliciet vermenigvuldigen
                Tok::Ident(_) | Tok::LParen => (BinOp::Mul, BP_JUX, true),
                _ => break,
            };
            if lbp < min_bp {
                break;
            }
            if !juxtaposition {
                self.advance();
            }
            let rhs = self.parse_expr(rbp)?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    /// Een door komma's gescheiden lijst tot en met `close`. Het openingsteken is al gelezen.
    fn parse_list(&mut self, close: Tok, open: usize, what: &str) -> Result<Vec<Expr>, Error> {
        let mut items = Vec::new();
        if self.peek().tok == close {
            self.advance();
            return Ok(items);
        }
        loop {
            items.push(self.parse_expr(0)?);
            match &self.peek().tok {
                Tok::Comma => {
                    self.advance();
                }
                t if *t == close => {
                    self.advance();
                    return Ok(items);
                }
                Tok::Eof => {
                    return Err(Error::parse(open, "dit haakje wordt nergens gesloten"));
                }
                _ => {
                    let sym = if close == Tok::RParen { "')'" } else { "']'" };
                    return Err(Error::parse(
                        self.peek().start,
                        format!("verwacht ',' of {sym} in {what}, maar vond {}", self.text(self.peek())),
                    ));
                }
            }
        }
    }

    /// `[1, 2; 3, 4]`. Het `[` is al gelezen.
    fn parse_matrix(&mut self, open: usize) -> Result<Expr, Error> {
        let mut rows: Vec<Vec<Expr>> = vec![Vec::new()];
        if self.peek().tok == Tok::RBracket {
            return Err(Error::parse(open, "een lege matrix kan niet"));
        }
        loop {
            rows.last_mut().unwrap().push(self.parse_expr(0)?);
            match self.peek().tok {
                Tok::Comma => {
                    self.advance();
                }
                Tok::Semicolon => {
                    self.advance();
                    rows.push(Vec::new());
                }
                Tok::RBracket => {
                    self.advance();
                    break;
                }
                Tok::Eof => return Err(Error::parse(open, "dit haakje wordt nergens gesloten")),
                _ => {
                    return Err(Error::parse(
                        self.peek().start,
                        format!(
                            "verwacht ',' (volgend element), ';' (volgende rij) of ']' in de matrix, maar vond {}",
                            self.text(self.peek())
                        ),
                    ))
                }
            }
        }
        let n = rows[0].len();
        if rows.iter().any(|r| r.len() != n) {
            return Err(Error::parse(open, "de rijen van deze matrix zijn niet even lang"));
        }
        Ok(Expr::Matrix(rows))
    }

    fn parse_prefix(&mut self) -> Result<Expr, Error> {
        let t = self.peek().clone();
        match t.tok {
            Tok::Num(v) => {
                self.advance();
                Ok(Expr::Num(v))
            }
            Tok::Imag(v) => {
                self.advance();
                Ok(Expr::Imag(v))
            }
            Tok::Str(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Tok::Ident(name) => {
                self.advance();
                if self.peek().tok != Tok::LParen {
                    return Ok(Expr::Var(name));
                }
                let open = self.advance().start;
                let mut args = self.parse_list(Tok::RParen, open, &format!("de argumenten van {name}"))?;
                // odesolve(t, b): t is de onafhankelijke variabele
                if name == "odesolve" {
                    return match (args.len(), args.first()) {
                        (2, Some(Expr::Var(v))) => Ok(Expr::Bind {
                            op: BindOp::OdeSolve,
                            body: Box::new(Expr::Num(0.0)),
                            var: v.clone(),
                            args: vec![args.remove(1)],
                        }),
                        _ => Err(Error::parse(t.start, "schrijfwijze: y := odesolve(t, eindwaarde)")),
                    };
                }
                if let Some((op, counts, syntax)) = binder(&name) {
                    // `sum(v)` en `prod(v)` met één argument zijn de som/het product van een vector;
                    // `root(27 m^3, 3)` (geen naam als tweede argument) is de n-de machtswortel
                    let vector_form = args.len() == 1 && matches!(op, BindOp::Sum | BindOp::Prod);
                    let nth_root = op == BindOp::Root && !matches!(args.get(1), Some(Expr::Var(_)));
                    if !vector_form && !nth_root {
                        if !counts.contains(&args.len()) {
                            return Err(Error::parse(t.start, format!("schrijfwijze: {syntax}")));
                        }
                        let var = match &args[1] {
                            Expr::Var(v) => v.clone(),
                            _ => {
                                return Err(Error::parse(
                                    t.start,
                                    format!("het tweede argument moet een naam zijn; schrijfwijze: {syntax}"),
                                ))
                            }
                        };
                        let body = args.remove(0);
                        args.remove(0);
                        return Ok(Expr::Bind {
                            op,
                            body: Box::new(body),
                            var,
                            args,
                        });
                    }
                }
                Ok(Expr::Call(name, args))
            }
            Tok::LParen => {
                self.advance();
                let e = self.parse_expr(0)?;
                if self.peek().tok == Tok::RParen {
                    self.advance();
                    Ok(e)
                } else if self.peek().tok == Tok::Eof {
                    Err(Error::parse(t.start, "dit haakje wordt nergens gesloten"))
                } else {
                    Err(self.unexpected())
                }
            }
            Tok::LBracket => {
                self.advance();
                self.parse_matrix(t.start)
            }
            Tok::Minus => {
                self.advance();
                Ok(Expr::Neg(Box::new(self.parse_expr(BP_NEG)?)))
            }
            Tok::Plus => {
                self.advance();
                self.parse_expr(BP_NEG)
            }
            Tok::Not => {
                self.advance();
                Ok(Expr::Not(Box::new(self.parse_expr(BP_NOT)?)))
            }
            _ => Err(Error::parse(
                t.start,
                format!("verwacht een getal, naam of haakje, maar vond {}", self.text(&t)),
            )),
        }
    }

    /// De rechterkant van een definitie: een uitdrukking of een bereik (`1..10`, `0, 0.5 .. 3`).
    fn parse_define_body(&mut self, allow_range: bool) -> Result<Expr, Error> {
        let first = self.parse_expr(0)?;
        let second = match self.peek().tok {
            Tok::Comma => {
                self.advance();
                Some(Box::new(self.parse_expr(0)?))
            }
            Tok::DotDot => None,
            _ => return Ok(first),
        };
        if self.peek().tok != Tok::DotDot {
            return Err(Error::parse(
                self.peek().start,
                "verwacht '..' (een bereik schrijf je als 'begin .. einde' of 'begin, volgende .. einde')",
            ));
        }
        let dots = self.advance().start;
        if !allow_range {
            return Err(Error::parse(
                dots,
                "een bereik kan alleen in een gewone definitie staan, zoals 'i := 1..10'",
            ));
        }
        let end = self.parse_expr(0)?;
        Ok(Expr::Range {
            start: Box::new(first),
            second,
            end: Box::new(end),
        })
    }
}

/// De kop van een definitie.
enum Head {
    Var(String),
    Fn(String, Vec<String>),
    /// `naam[...] :=`; `close` is de index van het sluitende `]`.
    Indexed(String, usize),
}

/// Herken `naam :=`, `naam(p1, p2) :=` en `naam[i] :=`. Geeft de kop en de index van het eerste
/// token van de body.
fn detect_define(toks: &[Token]) -> Option<(Head, usize)> {
    let name = match &toks.first()?.tok {
        Tok::Ident(n) => n.clone(),
        _ => return None,
    };
    match &toks.get(1)?.tok {
        Tok::Assign => Some((Head::Var(name), 2)),
        Tok::LParen => {
            let mut i = 2;
            let mut params = Vec::new();
            if toks.get(i)?.tok == Tok::RParen {
                i += 1;
            } else {
                loop {
                    match &toks.get(i)?.tok {
                        Tok::Ident(p) => params.push(p.clone()),
                        _ => return None,
                    }
                    i += 1;
                    match toks.get(i)?.tok {
                        Tok::Comma => i += 1,
                        Tok::RParen => {
                            i += 1;
                            break;
                        }
                        _ => return None,
                    }
                }
            }
            if toks.get(i)?.tok == Tok::Assign {
                Some((Head::Fn(name, params), i + 1))
            } else {
                None
            }
        }
        Tok::LBracket => {
            let mut depth = 0usize;
            for (i, t) in toks.iter().enumerate().skip(1) {
                match t.tok {
                    Tok::LBracket => depth += 1,
                    Tok::RBracket => {
                        depth -= 1;
                        if depth == 0 {
                            return if toks.get(i + 1)?.tok == Tok::Assign {
                                Some((Head::Indexed(name, i), i + 2))
                            } else {
                                None
                            };
                        }
                    }
                    Tok::Eof => return None,
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

/// De indices van `naam[...] :=`, tussen token 1 (`[`) en `close` (`]`).
fn parse_index_head(toks: &[Token], src: &str, close: usize) -> Result<Vec<Expr>, Error> {
    let mut ip = Parser {
        toks: &toks[..=close],
        src,
        pos: 2,
    };
    let idx = ip.parse_list(Tok::RBracket, toks[1].start, "de index")?;
    if idx.is_empty() || idx.len() > 2 {
        return Err(Error::parse(
            toks[1].start,
            "een index heeft 1 getal (v[i]) of 2 getallen (M[i, j])",
        ));
    }
    Ok(idx)
}

/// Een uitdrukking of bereik (voor `for k in 1..n`).
fn parse_iter_str(src: &str) -> Result<Expr, Error> {
    let toks = lex(src)?;
    if toks[0].tok == Tok::Eof {
        return Err(Error::parse(0, "lege uitdrukking"));
    }
    let mut p = Parser {
        toks: &toks,
        src,
        pos: 0,
    };
    let e = p.parse_define_body(true)?;
    p.expect_eof()?;
    Ok(e)
}

// ---------------------------------------------------------------------------------------------
// Programmablokken

/// Eén niet-lege regel van een blok: regelnummer binnen de regio, inspringing en tekst.
struct BLine<'a> {
    no: usize,
    indent: usize,
    text: &'a str,
}

fn block_err(no: usize, msg: impl std::fmt::Display) -> Error {
    Error::parse(0, format!("regel {no} van het blok: {msg}"))
}

fn indent_of(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// Parseer de regels van een blok (zonder de kop `... := {` en de afsluitende `}`).
/// `first_no` is het regelnummer van de eerste regel binnen de regio (voor foutmeldingen).
pub fn parse_block(lines: &[&str], first_no: usize) -> Result<Vec<BStmt>, Error> {
    let lines: Vec<BLine> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            let code = l.find("//").map_or(*l, |k| &l[..k]);
            (!code.trim().is_empty()).then(|| BLine {
                no: first_no + i,
                indent: indent_of(code),
                text: code.trim(),
            })
        })
        .collect();
    if lines.is_empty() {
        return Err(block_err(first_no, "het blok is leeg"));
    }
    let mut pos = 0;
    let stmts = parse_seq(&lines, &mut pos, lines[0].indent)?;
    if pos < lines.len() {
        return Err(block_err(
            lines[pos].no,
            "deze regel springt minder ver in dan de eerste regel van het blok",
        ));
    }
    Ok(stmts)
}

fn parse_seq(lines: &[BLine], pos: &mut usize, indent: usize) -> Result<Vec<BStmt>, Error> {
    let mut out = Vec::new();
    while *pos < lines.len() {
        let l = &lines[*pos];
        if l.indent < indent {
            break;
        }
        if l.indent > indent {
            return Err(block_err(
                l.no,
                "deze regel springt verder in, maar de regel erboven opent geen blok (if, else, for, while)",
            ));
        }
        out.push(parse_line_stmt(lines, pos, indent)?);
    }
    Ok(out)
}

/// De ingesprongen regels onder een kop (`if`, `for`, ...). `*pos` staat al op de eerste ervan.
fn body_of(lines: &[BLine], pos: &mut usize, indent: usize, no: usize, what: &str) -> Result<Vec<BStmt>, Error> {
    match lines.get(*pos) {
        Some(l) if l.indent > indent => parse_seq(lines, pos, l.indent),
        _ => Err(block_err(no, format!("na '{what}' moet minstens één ingesprongen regel volgen"))),
    }
}

fn parse_line_stmt(lines: &[BLine], pos: &mut usize, indent: usize) -> Result<BStmt, Error> {
    let l = &lines[*pos];
    let (no, text) = (l.no, l.text);
    let wrap = |e: Error| block_err(no, e);
    let toks = lex(text).map_err(wrap)?;
    let kw = match &toks[0].tok {
        Tok::Ident(s) => s.as_str(),
        _ => "",
    };
    let rest = |k: usize| &text[toks[k].start..];
    // `if` en `while` openen alleen een blok als de volgende regel verder inspringt;
    // anders is `if(v, a, b)` gewoon de functie
    let opens_block = lines.get(*pos + 1).is_some_and(|n| n.indent > indent);
    let expr = |s: &str| parse_expr_str(s).map_err(wrap);
    match kw {
        "if" if opens_block => {
            let cond = expr(rest(1))?;
            *pos += 1;
            let mut branches = vec![(cond, body_of(lines, pos, indent, no, "if")?)];
            let mut els = None;
            while let Some(e) = lines.get(*pos).filter(|e| e.indent == indent) {
                let et = lex(e.text).map_err(|x| block_err(e.no, x))?;
                if et[0].tok != Tok::Ident("else".into()) {
                    break;
                }
                let eno = e.no;
                if et[1].tok == Tok::Ident("if".into()) {
                    let c = parse_expr_str(&e.text[et[2].start..]).map_err(|x| block_err(eno, x))?;
                    *pos += 1;
                    branches.push((c, body_of(lines, pos, indent, eno, "else if")?));
                } else if et[1].tok == Tok::Eof {
                    *pos += 1;
                    els = Some(body_of(lines, pos, indent, eno, "else")?);
                    break;
                } else {
                    return Err(block_err(eno, "na 'else' komt niets, of 'if' met een voorwaarde"));
                }
            }
            Ok(BStmt::If { branches, els })
        }
        "else" => Err(block_err(no, "'else' zonder bijbehorende 'if' erboven (op dezelfde inspringing)")),
        "while" if opens_block => {
            let cond = expr(rest(1))?;
            *pos += 1;
            let body = body_of(lines, pos, indent, no, "while")?;
            Ok(BStmt::While { cond, body })
        }
        "for" => {
            let var = match (&toks[1].tok, &toks[2].tok) {
                (Tok::Ident(v), Tok::Ident(kw)) if kw == "in" || kw == "∈" => v.clone(),
                _ => return Err(block_err(no, "schrijfwijze: for k in 1..n (of: for k ∈ v)")),
            };
            let iter = parse_iter_str(rest(3)).map_err(wrap)?;
            *pos += 1;
            let body = body_of(lines, pos, indent, no, "for")?;
            Ok(BStmt::For { var, iter, body })
        }
        "break" | "continue" if toks[1].tok == Tok::Eof => {
            *pos += 1;
            Ok(if kw == "break" { BStmt::Break } else { BStmt::Continue })
        }
        "return" => {
            if toks[1].tok == Tok::Eof {
                return Err(block_err(no, "na 'return' hoort een waarde"));
            }
            let e = expr(rest(1))?;
            *pos += 1;
            Ok(BStmt::Return(e))
        }
        _ => {
            let stmt = match detect_define(&toks) {
                Some((Head::Var(name), body_start)) => BStmt::Assign {
                    name,
                    index: None,
                    value: expr(rest(body_start))?,
                },
                Some((Head::Indexed(name, close), body_start)) => BStmt::Assign {
                    name,
                    index: Some(parse_index_head(&toks, text, close).map_err(wrap)?),
                    value: expr(rest(body_start))?,
                },
                Some((Head::Fn(..), _)) => {
                    return Err(block_err(no, "een functie definieer je buiten het blok, niet erin"))
                }
                None => BStmt::Expr(expr(text)?),
            };
            *pos += 1;
            Ok(stmt)
        }
    }
}

/// `f(x) := {` ... `}` over meerdere regels.
fn parse_block_stmt(src: &str) -> Result<Stmt, Error> {
    let lines: Vec<&str> = src.lines().collect();
    let head = lines[0].trim();
    let head = head.strip_suffix('{').ok_or_else(|| {
        Error::parse(0, "een regio over meerdere regels begint met 'naam := {' of 'f(x) := {'")
    })?;
    let last = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .filter(|&i| i > 0 && lines[i].trim() == "}")
        .ok_or_else(|| Error::parse(0, "het blok wordt niet afgesloten met een regel '}'"))?;
    let toks = lex(head)?;
    let (head, _) = detect_define(&toks)
        .filter(|(_, b)| toks[*b].tok == Tok::Eof)
        .ok_or_else(|| Error::parse(0, "voor '{' hoort 'naam :=' of 'f(x) :='"))?;
    let (name, params, index) = match head {
        Head::Var(name) => (name, None, None),
        Head::Fn(name, params) => (name, Some(params), None),
        Head::Indexed(name, close) => (name, None, Some(parse_index_head(&toks, lines[0].trim(), close)?)),
    };
    let body = Expr::Block(parse_block(&lines[1..last], 2)?);
    Ok(Stmt::Define {
        name,
        params,
        index,
        body,
        show: None,
    })
}

/// Parseer één regel wiskunde (of een programmablok over meerdere regels).
///
/// * `naam := uitdrukking`, `f(x, y) := uitdrukking` en `v[i] := uitdrukking` zijn definities.
/// * `i := 1..10` en `x := 0, 0.5 .. 3` definiëren een bereik.
/// * `uitdrukking =` toont het resultaat; `uitdrukking = eenheid` toont het in die eenheid.
/// * Met `allow_bare` mag het `=` ontbreken (handig in de REPL).
pub fn parse_stmt(src: &str, allow_bare: bool) -> Result<Stmt, Error> {
    if src.trim().contains('\n') {
        return parse_block_stmt(src.trim());
    }
    let toks = lex(src)?;
    if toks[0].tok == Tok::Eof {
        return Err(Error::parse(0, "lege invoer"));
    }
    if let Some((head, body_start)) = detect_define(&toks) {
        let mut p = Parser {
            toks: &toks,
            src,
            pos: body_start,
        };
        if p.peek().tok == Tok::Eof {
            return Err(Error::parse(src.len(), "na ':=' ontbreekt een uitdrukking"));
        }
        let (name, params, index) = match head {
            Head::Var(name) => (name, None, None),
            Head::Fn(name, params) => (name, Some(params), None),
            Head::Indexed(name, close) => (name, None, Some(parse_index_head(&toks, src, close)?)),
        };
        let allow_range = params.is_none() && index.is_none();
        let body = p.parse_define_body(allow_range)?;
        // `a := expr =` of `a := expr = eenheid`: definiëren en meteen tonen (zoals in Mathcad)
        let mut show = None;
        if p.peek().tok == Tok::Eq {
            if params.is_some() || index.is_some() {
                return Err(Error::parse(p.peek().start, "een functie of een element kan niet meteen getoond worden; zet 'f(1) =' in een eigen regio"));
            }
            let head_src = src[..p.peek().start].trim().to_string();
            p.advance();
            let unit = if p.peek().tok == Tok::Eof {
                None
            } else {
                let ustart = p.peek().start;
                let ue = p.parse_expr(0)?;
                let uend = toks[p.pos.saturating_sub(1)].end;
                Some((ue, src[ustart..uend].trim().to_string()))
            };
            show = Some(crate::ast::Show { unit, head_src });
        }
        p.expect_eof()?;
        return Ok(Stmt::Define {
            name,
            params,
            index,
            body,
            show,
        });
    }

    let mut p = Parser {
        toks: &toks,
        src,
        pos: 0,
    };
    let expr = p.parse_expr(0)?;
    let lhs_end = toks[p.pos.saturating_sub(1)].end;
    let lhs_src = src[toks[0].start..lhs_end].trim().to_string();
    match p.peek().tok {
        Tok::Eq => {
            p.advance();
            if p.peek().tok == Tok::Eof {
                return Ok(Stmt::Eval {
                    expr,
                    lhs_src,
                    unit: None,
                    bare: false,
                });
            }
            let ustart = p.peek().start;
            let ue = p.parse_expr(0)?;
            let uend = toks[p.pos.saturating_sub(1)].end;
            p.expect_eof()?;
            Ok(Stmt::Eval {
                expr,
                lhs_src,
                unit: Some((ue, src[ustart..uend].trim().to_string())),
                bare: false,
            })
        }
        // symbolisch: `expr →`, `expr → simplify`, `vergelijking → solve, x`, `expr → substitute, x = 2`
        Tok::Arrow => {
            p.advance();
            let kw_start = p.peek().start;
            let op = match p.peek().tok.clone() {
                Tok::Eof => crate::ast::SymOp::Evaluate,
                Tok::Ident(kw) => {
                    p.advance();
                    let var = |p: &mut Parser| -> Result<String, Error> {
                        if p.peek().tok != Tok::Comma {
                            return Err(Error::parse(p.peek().start, format!("na '{kw}' hoort een komma en de naam, zoals '{kw}, x'")));
                        }
                        p.advance();
                        match p.peek().tok.clone() {
                            Tok::Ident(x) => {
                                p.advance();
                                Ok(x)
                            }
                            _ => Err(Error::parse(p.peek().start, "verwacht een naam")),
                        }
                    };
                    match kw.as_str() {
                        "simplify" | "vereenvoudig" => crate::ast::SymOp::Simplify,
                        "expand" | "uitwerken" => crate::ast::SymOp::Expand,
                        "factor" | "ontbind" => crate::ast::SymOp::Factor,
                        "solve" | "los" => crate::ast::SymOp::Solve(var(&mut p)?),
                        "substitute" | "vervang" => {
                            let x = var(&mut p)?;
                            if !matches!(p.peek().tok, Tok::Eq | Tok::EqEq) {
                                return Err(Error::parse(p.peek().start, "na 'substitute, x' hoort '= waarde'"));
                            }
                            p.advance();
                            crate::ast::SymOp::Substitute(x, p.parse_expr(0)?)
                        }
                        other => {
                            return Err(Error::parse(
                                kw_start,
                                format!("onbekend sleutelwoord '{other}' na →; gebruik simplify, expand, factor, solve, x of substitute, x = …"),
                            ))
                        }
                    }
                }
                _ => return Err(p.unexpected()),
            };
            p.expect_eof()?;
            Ok(Stmt::Symbolic { expr, op, lhs_src })
        }
        Tok::Eof if allow_bare => Ok(Stmt::Eval {
            expr,
            lhs_src,
            unit: None,
            bare: true,
        }),
        Tok::Eof => Err(Error::parse(
            src.len(),
            "verwacht ':=' (definitie) of '=' (resultaat tonen)",
        )),
        Tok::Assign => Err(Error::parse(
            p.peek().start,
            "links van ':=' moet een naam staan, een functie zoals f(x, y) of een element zoals v[i]",
        )),
        Tok::DotDot => Err(Error::parse(
            p.peek().start,
            "een bereik kan alleen in een definitie staan, zoals 'i := 1..10'",
        )),
        _ => Err(p.unexpected()),
    }
}

/// Parseer een losse uitdrukking (zonder `:=` of `=`).
pub fn parse_expr_str(src: &str) -> Result<Expr, Error> {
    let toks = lex(src)?;
    if toks[0].tok == Tok::Eof {
        return Err(Error::parse(0, "lege uitdrukking"));
    }
    let mut p = Parser {
        toks: &toks,
        src,
        pos: 0,
    };
    let e = p.parse_expr(0)?;
    p.expect_eof()?;
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use BinOp::*;

    fn num(v: f64) -> Box<Expr> {
        Box::new(Expr::Num(v))
    }
    fn var(n: &str) -> Box<Expr> {
        Box::new(Expr::Var(n.into()))
    }

    #[test]
    fn voorrang() {
        // 1 + 2 * 3 ^ 2
        let e = parse_expr_str("1 + 2 * 3 ^ 2").unwrap();
        assert_eq!(
            e,
            Expr::Bin(
                Add,
                num(1.0),
                Box::new(Expr::Bin(
                    Mul,
                    num(2.0),
                    Box::new(Expr::Bin(Pow, num(3.0), num(2.0)))
                ))
            )
        );
    }

    #[test]
    fn macht_is_rechts_associatief_en_min_bindt_zwakker() {
        let e = parse_expr_str("2^3^2").unwrap();
        assert_eq!(
            e,
            Expr::Bin(
                Pow,
                num(2.0),
                Box::new(Expr::Bin(Pow, num(3.0), num(2.0)))
            )
        );
        let e = parse_expr_str("-2^2").unwrap();
        assert_eq!(
            e,
            Expr::Neg(Box::new(Expr::Bin(Pow, num(2.0), num(2.0))))
        );
        // exponent mag een min hebben
        assert!(parse_expr_str("2^-3").is_ok());
    }

    #[test]
    fn juxtapositie() {
        // 5 m/s  =  (5 m) / s
        let e = parse_expr_str("5 m/s").unwrap();
        assert_eq!(
            e,
            Expr::Bin(
                Div,
                Box::new(Expr::Bin(Mul, num(5.0), var("m"))),
                var("s")
            )
        );
        // getal met eenheid als geheel: 230 V / 50 Ω = (230 V) / (50 Ω)
        let e = parse_expr_str("230 V / 50 Ω").unwrap();
        assert_eq!(
            e,
            Expr::Bin(Div, Box::new(Expr::Bin(Mul, num(230.0), var("V"))), Box::new(Expr::Bin(Mul, num(50.0), var("Ω"))))
        );
        // 5 m^2 = 5 (m^2)
        let e = parse_expr_str("5 m^2").unwrap();
        assert_eq!(
            e,
            Expr::Bin(
                Mul,
                num(5.0),
                Box::new(Expr::Bin(Pow, var("m"), num(2.0)))
            )
        );
    }

    #[test]
    fn definities() {
        match parse_stmt("a := 5 m", false).unwrap() {
            Stmt::Define { name, params, .. } => {
                assert_eq!(name, "a");
                assert!(params.is_none());
            }
            s => panic!("{s:?}"),
        }
        match parse_stmt("f(x, y) := x^2 + y", false).unwrap() {
            Stmt::Define { name, params, .. } => {
                assert_eq!(name, "f");
                assert_eq!(params.unwrap(), vec!["x".to_string(), "y".to_string()]);
            }
            s => panic!("{s:?}"),
        }
        match parse_stmt("M[i, j] := i + j", false).unwrap() {
            Stmt::Define { name, index, .. } => {
                assert_eq!(name, "M");
                assert_eq!(index.unwrap().len(), 2);
            }
            s => panic!("{s:?}"),
        }
    }

    #[test]
    fn bereiken() {
        match parse_stmt("i := 1..10", false).unwrap() {
            Stmt::Define { body: Expr::Range { second: None, .. }, .. } => {}
            s => panic!("{s:?}"),
        }
        match parse_stmt("x := 0 m, 0.5 m .. 3 m", false).unwrap() {
            Stmt::Define { body: Expr::Range { second: Some(_), .. }, .. } => {}
            s => panic!("{s:?}"),
        }
        assert!(parse_stmt("f(x) := 1..x", false).is_err());
        assert!(parse_stmt("1..3 =", false).is_err());
    }

    #[test]
    fn vergelijkingen_en_logica() {
        // 0 < x ≤ 1 is één ketting
        match parse_expr_str("0 < x ≤ 1").unwrap() {
            Expr::Cmp(ops, xs) => {
                assert_eq!(ops, vec![CmpOp::Lt, CmpOp::Le]);
                assert_eq!(xs.len(), 3);
            }
            e => panic!("{e:?}"),
        }
        // and bindt sterker dan or, vergelijken sterker dan and
        match parse_expr_str("a < 1 or b > 2 and c == 3").unwrap() {
            Expr::Or(_, r) => assert!(matches!(*r, Expr::And(_, _))),
            e => panic!("{e:?}"),
        }
        // optellen bindt sterker dan vergelijken
        match parse_expr_str("a + 1 < b").unwrap() {
            Expr::Cmp(_, xs) => assert!(matches!(xs[0], Expr::Bin(Add, _, _))),
            e => panic!("{e:?}"),
        }
        match parse_expr_str("not a < b").unwrap() {
            Expr::Not(x) => assert!(matches!(*x, Expr::Cmp(_, _))),
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn matrices_en_indices() {
        match parse_expr_str("[1, 2; 3, 4]").unwrap() {
            Expr::Matrix(rows) => assert_eq!((rows.len(), rows[0].len()), (2, 2)),
            e => panic!("{e:?}"),
        }
        assert!(parse_expr_str("[1, 2; 3]").is_err());
        assert!(parse_expr_str("[]").is_err());
        match parse_expr_str("-v[1]^2").unwrap() {
            Expr::Neg(x) => match *x {
                Expr::Bin(Pow, b, _) => assert!(matches!(*b, Expr::Index(_, _))),
                e => panic!("{e:?}"),
            },
            e => panic!("{e:?}"),
        }
        match parse_expr_str("3!").unwrap() {
            Expr::Call(n, _) => assert_eq!(n, "factorial"),
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn gebonden_variabelen() {
        match parse_expr_str("sum(i^2, i, 1, n)").unwrap() {
            Expr::Bind { op: BindOp::Sum, var, args, .. } => {
                assert_eq!(var, "i");
                assert_eq!(args.len(), 2);
            }
            e => panic!("{e:?}"),
        }
        // sum(v) blijft een gewone functieaanroep
        assert!(matches!(parse_expr_str("sum(v)").unwrap(), Expr::Call(_, _)));
        assert!(parse_expr_str("integral(x^2, 3, 0, 1)").is_err());
        assert!(parse_expr_str("deriv(x^2, x)").is_err());
        // de gebonden variabele telt niet als gebruikte naam
        let names = parse_expr_str("integral(a x^2, x, 0, b)").unwrap().names();
        assert_eq!(names.into_iter().collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn resultaatregels() {
        match parse_stmt("M.max = kN*m", false).unwrap() {
            Stmt::Eval { lhs_src, unit, bare, .. } => {
                assert_eq!(lhs_src, "M.max");
                assert_eq!(unit.unwrap().1, "kN*m");
                assert!(!bare);
            }
            s => panic!("{s:?}"),
        }
        assert!(parse_stmt("a + b", false).is_err());
        assert!(matches!(
            parse_stmt("a + b", true).unwrap(),
            Stmt::Eval { bare: true, .. }
        ));
    }

    #[test]
    fn foutmeldingen() {
        assert!(parse_stmt("5 := a", false).is_err());
        assert!(parse_stmt("a := ", false).is_err());
        assert!(parse_stmt("(1 + 2", true).is_err());
        assert!(parse_stmt("1 + * 2", true).is_err());
        assert!(parse_stmt("f(1, 2", true).is_err());
        // twee getallen achter elkaar is geen impliciet product
        assert!(parse_stmt("2 3 =", false).is_err());
    }

    #[test]
    fn namen_verzamelen() {
        let s = parse_stmt("f(x) := a * x + sin(b)", false).unwrap();
        let uses: Vec<String> = s.uses().into_iter().collect();
        assert_eq!(uses, vec!["a", "b", "sin"]);
    }
}
