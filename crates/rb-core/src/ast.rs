use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// Vergelijkingsoperatoren. Een vergelijking geeft 1 (waar) of 0 (onwaar), zoals in Mathcad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

/// Operatoren die een eigen variabele binden: `sum(i^2, i, 1, 10)`, `integral(x^2, x, 0, 1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindOp {
    /// `sum(uitdrukking, i, van, tot)`
    Sum,
    /// `prod(uitdrukking, i, van, tot)`
    Prod,
    /// `integral(uitdrukking, x, a, b)`
    Integral,
    /// `deriv(uitdrukking, x, punt)` of `deriv(uitdrukking, x, punt, orde)`
    Deriv,
    /// `root(uitdrukking, x)` (startwaarde = huidige x) of `root(uitdrukking, x, a, b)`
    Root,
    /// `odesolve(t, b)` in een Given-blok; de body is niet gebruikt
    OdeSolve,
}

/// Een getabelleerde functie (uitkomst van `odesolve`): kubische Hermite-interpolatie.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// Knooppunten (SI), oplopend of aflopend.
    pub ts: Vec<f64>,
    /// Functiewaarden en afgeleiden in de knooppunten (SI).
    pub ys: Vec<f64>,
    pub ds: Vec<f64>,
    pub tdim: rb_units::Dim,
    pub ydim: rb_units::Dim,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    /// Imaginair getal, zoals `2i` of `3j`.
    Imag(f64),
    /// Tekst: `"abc"`.
    Str(String),
    Var(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    /// Een ketting van vergelijkingen: `0 < x ≤ 1` is (0 < x) en (x ≤ 1).
    Cmp(Vec<CmpOp>, Vec<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    /// `v[i]` of `M[i, j]`
    Index(Box<Expr>, Vec<Expr>),
    /// `[1, 2; 3, 4]`: rijen gescheiden door `;`, elementen door `,`.
    Matrix(Vec<Vec<Expr>>),
    Bind {
        op: BindOp,
        body: Box<Expr>,
        var: String,
        args: Vec<Expr>,
    },
    /// `1..10` of `0, 0.5 .. 3` (alleen als hele rechterkant van een definitie).
    Range {
        start: Box<Expr>,
        second: Option<Box<Expr>>,
        end: Box<Expr>,
    },
    /// Een programmablok (`f(n) := {` ... `}`) met lokale variabelen.
    Block(Vec<BStmt>),
    /// Interpolatie in een tabel (de body van een functie uit `odesolve`).
    Interp(std::sync::Arc<Table>, Box<Expr>),
}

/// Eén regel in een programmablok.
#[derive(Debug, Clone, PartialEq)]
pub enum BStmt {
    /// `x := ...`, `x ← ...` of `v[i] := ...` (lokaal)
    Assign {
        name: String,
        index: Option<Vec<Expr>>,
        value: Expr,
    },
    /// `if` / `else if` / `else`
    If {
        branches: Vec<(Expr, Vec<BStmt>)>,
        els: Option<Vec<BStmt>>,
    },
    /// `for k in 1..n` (of `for k ∈ v`)
    For {
        var: String,
        iter: Expr,
        body: Vec<BStmt>,
    },
    While {
        cond: Expr,
        body: Vec<BStmt>,
    },
    Break,
    Continue,
    Return(Expr),
    Expr(Expr),
}

impl BStmt {
    /// Verzamel de namen die deze regel van buiten het blok leest. `bound` bevat de namen die op dit
    /// punt lokaal zijn; een gewone toekenning maakt een naam vanaf daar lokaal.
    fn collect_names(&self, bound: &mut BTreeSet<String>, out: &mut BTreeSet<String>) {
        let body = |stmts: &[BStmt], bound: &BTreeSet<String>, out: &mut BTreeSet<String>| {
            let mut inner = bound.clone();
            for s in stmts {
                s.collect_names(&mut inner, out);
            }
        };
        match self {
            BStmt::Assign { name, index, value } => {
                value.collect_names(bound, out);
                match index {
                    None => {
                        bound.insert(name.clone());
                    }
                    Some(idx) => {
                        for i in idx {
                            i.collect_names(bound, out);
                        }
                        for n in [name.as_str(), "ORIGIN"] {
                            if !bound.contains(n) {
                                out.insert(n.to_string());
                            }
                        }
                    }
                }
            }
            BStmt::If { branches, els } => {
                for (c, b) in branches {
                    c.collect_names(bound, out);
                    body(b, bound, out);
                }
                if let Some(b) = els {
                    body(b, bound, out);
                }
            }
            BStmt::For { var, iter, body: b } => {
                iter.collect_names(bound, out);
                let mut inner = bound.clone();
                inner.insert(var.clone());
                body(b, &inner, out);
            }
            BStmt::While { cond, body: b } => {
                cond.collect_names(bound, out);
                body(b, bound, out);
            }
            BStmt::Break | BStmt::Continue => {}
            BStmt::Return(e) | BStmt::Expr(e) => e.collect_names(bound, out),
        }
    }
}

impl Expr {
    /// Verzamel alle namen die in deze uitdrukking voorkomen (variabelen en aangeroepen functies),
    /// behalve de namen in `bound` (bijvoorbeeld functieparameters).
    pub fn collect_names(&self, bound: &BTreeSet<String>, out: &mut BTreeSet<String>) {
        match self {
            Expr::Num(_) | Expr::Imag(_) | Expr::Str(_) => {}
            Expr::Var(n) => {
                if !bound.contains(n) {
                    out.insert(n.clone());
                }
            }
            Expr::Neg(x) | Expr::Not(x) => x.collect_names(bound, out),
            Expr::Bin(_, a, b) | Expr::And(a, b) | Expr::Or(a, b) => {
                a.collect_names(bound, out);
                b.collect_names(bound, out);
            }
            Expr::Cmp(_, xs) => {
                for x in xs {
                    x.collect_names(bound, out);
                }
            }
            Expr::Call(name, args) => {
                if !bound.contains(name) {
                    out.insert(name.clone());
                    // toevalsgetallen hangen af van het zaad SEED (als het blad dat definieert)
                    if matches!(name.as_str(), "rnd" | "runif" | "rnorm") {
                        out.insert("SEED".to_string());
                    }
                }
                for a in args {
                    a.collect_names(bound, out);
                }
            }
            Expr::Index(base, idx) => {
                base.collect_names(bound, out);
                for i in idx {
                    i.collect_names(bound, out);
                }
                // indices hangen af van de instelling ORIGIN (standaard 0)
                if !bound.contains("ORIGIN") {
                    out.insert("ORIGIN".to_string());
                }
            }
            Expr::Matrix(rows) => {
                for x in rows.iter().flatten() {
                    x.collect_names(bound, out);
                }
            }
            Expr::Bind { op, body, var, args } => {
                for a in args {
                    a.collect_names(bound, out);
                }
                // root(f(x), x) leest de startwaarde van x van buiten
                if *op == BindOp::Root && args.is_empty() && !bound.contains(var) {
                    out.insert(var.clone());
                }
                let mut inner = bound.clone();
                inner.insert(var.clone());
                body.collect_names(&inner, out);
            }
            Expr::Range { start, second, end } => {
                start.collect_names(bound, out);
                if let Some(s) = second {
                    s.collect_names(bound, out);
                }
                end.collect_names(bound, out);
            }
            Expr::Block(stmts) => {
                let mut inner = bound.clone();
                for s in stmts {
                    s.collect_names(&mut inner, out);
                }
            }
            Expr::Interp(_, arg) => arg.collect_names(bound, out),
        }
    }

    /// Alle vrije namen (zonder gebonden variabelen).
    pub fn names(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        self.collect_names(&BTreeSet::new(), &mut out);
        out
    }
}

/// Het tonen van een waarde achter een definitie (`a := 2 m = cm`).
#[derive(Debug, Clone, PartialEq)]
pub struct Show {
    /// De weergave-eenheid, met de tekst zoals getypt.
    pub unit: Option<(Expr, String)>,
    /// De bron vóór de `=` (`a := 2 m`), voor de weergave als tekst.
    pub head_src: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `a := expr`, `f(x, y) := expr` of `v[i] := expr`
    Define {
        name: String,
        params: Option<Vec<String>>,
        /// Indices bij een elementdefinitie (`v[i] := ...`, `M[i, j] := ...`).
        index: Option<Vec<Expr>>,
        body: Expr,
        /// `a := expr =` of `a := expr = eenheid`: de waarde meteen tonen, zoals in Mathcad.
        show: Option<Show>,
    },
    /// `expr =` of `expr = eenheid`. `bare` is waar als er geen `=` achter stond (alleen in de REPL).
    Eval {
        expr: Expr,
        lhs_src: String,
        unit: Option<(Expr, String)>,
        bare: bool,
    },
    /// `expr →` of `expr → sleutelwoord`: symbolisch uitrekenen, zoals de pijl in Mathcad.
    Symbolic { expr: Expr, op: SymOp, lhs_src: String },
}

/// Wat de pijl → doet.
#[derive(Debug, Clone, PartialEq)]
pub enum SymOp {
    /// `→`: uitrekenen met de bekende waarden en vereenvoudigen
    Evaluate,
    /// `→ simplify`
    Simplify,
    /// `→ expand`
    Expand,
    /// `→ factor`
    Factor,
    /// `→ solve, x` (de uitdrukking is een vergelijking `a == b`, of `= 0`)
    Solve(String),
    /// `→ substitute, x = waarde`
    Substitute(String, Expr),
}

impl SymOp {
    /// Het sleutelwoord zoals het achter de pijl staat (leeg voor gewoon uitrekenen).
    pub fn keyword(&self) -> String {
        match self {
            SymOp::Evaluate => String::new(),
            SymOp::Simplify => "simplify".into(),
            SymOp::Expand => "expand".into(),
            SymOp::Factor => "factor".into(),
            SymOp::Solve(x) => format!("solve, {x}"),
            SymOp::Substitute(x, v) => format!("substitute, {x} = {}", crate::symbolic::print(v)),
        }
    }
}

impl Stmt {
    /// De naam die door deze regel wordt gedefinieerd.
    pub fn defines(&self) -> Option<&str> {
        match self {
            Stmt::Define { name, .. } => Some(name),
            Stmt::Eval { .. } | Stmt::Symbolic { .. } => None,
        }
    }

    pub fn is_function(&self) -> bool {
        matches!(self, Stmt::Define { params: Some(_), .. })
    }

    /// Vult deze regel een bestaande vector of matrix aan (`v[i] := ...`)? Dan hangt hij af van de
    /// vorige definitie van dezelfde naam.
    pub fn updates_existing(&self) -> bool {
        matches!(self, Stmt::Define { index: Some(_), .. })
    }

    /// Alle namen die deze regel gebruikt (zonder de eigen parameters).
    pub fn uses(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        match self {
            Stmt::Define {
                params,
                index,
                body,
                show,
                ..
            } => {
                let bound: BTreeSet<String> = params
                    .as_ref()
                    .map(|p| p.iter().cloned().collect())
                    .unwrap_or_default();
                body.collect_names(&bound, &mut out);
                if let Some(Show { unit: Some((u, _)), .. }) = show {
                    u.collect_names(&BTreeSet::new(), &mut out);
                }
                if let Some(idx) = index {
                    for i in idx {
                        i.collect_names(&bound, &mut out);
                    }
                    out.insert("ORIGIN".to_string());
                }
            }
            Stmt::Eval { expr, unit, .. } => {
                let none = BTreeSet::new();
                expr.collect_names(&none, &mut out);
                if let Some((u, _)) = unit {
                    u.collect_names(&none, &mut out);
                }
            }
            // symbolisch: de onbekende van solve/substitute is een symbool, geen gebruikte waarde
            Stmt::Symbolic { expr, op, .. } => {
                let mut bound = BTreeSet::new();
                if let SymOp::Solve(x) | SymOp::Substitute(x, _) = op {
                    bound.insert(x.clone());
                }
                expr.collect_names(&bound, &mut out);
                if let SymOp::Substitute(_, v) = op {
                    v.collect_names(&BTreeSet::new(), &mut out);
                }
            }
        }
        out
    }

    /// Is dit symbolisch rekenen (→)? Dan mogen namen ook onbekend zijn: het zijn symbolen.
    pub fn is_symbolic(&self) -> bool {
        matches!(self, Stmt::Symbolic { .. })
    }
}
