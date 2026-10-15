use crate::graph::{Graph, Warning};
use crate::parse::{mark_given_blocks, parse_sheet, sort_regions, Kind, Region, SweepSpec};
use crate::plot::{eval_plot, PlotData};
use rb_core::{format_rows, prefix_lines, render_value, with_stack, Defined, Display, Env, Error, Outcome, Stmt, Value};
use rb_units::{format_quantity, Quantity};
use std::collections::BTreeSet;

/// Wat een regio na het rekenen laat zien.
#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// Niets extra's (tekst, kop, definitie).
    None,
    /// De waarde van een resultaatregel, bijvoorbeeld `12.5 MPa`. Een matrix beslaat meerdere regels.
    Value(String),
    /// De rijen van een sweep of van een resultaat over een bereik.
    Table(Vec<String>),
    /// Een grafiek, al in weergave-eenheden.
    Plot(PlotData),
    /// De uitkomst van symbolisch rekenen (`→`): een uitdrukking.
    Symbolic(rb_core::Expr),
    /// Een fout. `col` is de kolom (in tekens) in de bronregel als de fout daar een plek heeft.
    Error { message: String, col: Option<usize> },
}

#[derive(Debug, Clone)]
struct Cached {
    output: Output,
    /// Wat de regio definieert (een tabel definieert meerdere namen).
    defined: Vec<Defined>,
    /// De waarde van een resultaatregel en hoe die getoond wordt (voor de formuleweergave).
    result: Option<(Value, Option<Display>)>,
}

impl Cached {
    fn none() -> Cached {
        Cached {
            output: Output::None,
            defined: Vec::new(),
            result: None,
        }
    }

    fn error(message: String, col: Option<usize>) -> Cached {
        Cached {
            output: Output::Error { message, col },
            defined: Vec::new(),
            result: None,
        }
    }
}

/// Een werkblad: regio's, hun afhankelijkheden en de laatst berekende resultaten.
pub struct Sheet {
    regions: Vec<Region>,
    graph: Graph,
    cache: Vec<Cached>,
    /// Bestandsfuncties (READCSV, WRITEEXCEL, ...), met paden relatief aan de map van het blad.
    io: std::sync::Arc<crate::io::FileIo>,
    /// De bestandsnaam (zonder map), voor {bestand} in kop- en voetteksten.
    name: String,
}

impl Sheet {
    /// Lees een werkblad en reken het volledig door. Bestanden (READCSV, ...) zijn relatief aan de
    /// huidige map; gebruik [`Sheet::load_in`] voor de map van het blad.
    pub fn load(src: &str) -> Sheet {
        Sheet::load_in(src, std::path::Path::new("."))
    }

    /// De bestandsnaam (zonder map); leeg voor een nieuw blad.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Zet de bestandsnaam (voor {bestand} in kop- en voetteksten).
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// De map waartegen bestandspaden (READCSV, ...) relatief zijn.
    pub fn dir(&self) -> &std::path::Path {
        &self.io.base
    }

    /// Als [`Sheet::load`], met bestandspaden relatief aan `dir` (de map van het blad).
    pub fn load_in(src: &str, dir: &std::path::Path) -> Sheet {
        let regions = parse_sheet(src);
        let graph = Graph::build(&regions);
        let cache = vec![Cached::none(); regions.len()];
        let mut sheet = Sheet {
            regions,
            graph,
            cache,
            io: std::sync::Arc::new(crate::io::FileIo { base: dir.to_path_buf() }),
            name: String::new(),
        };
        sheet.recalc_all();
        sheet
    }

    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// De waarde en weergave van resultaatregel `i` (als die er een heeft en foutloos is).
    pub fn result(&self, i: usize) -> Option<(&Value, &Option<Display>)> {
        self.cache[i].result.as_ref().map(|(v, d)| (v, d))
    }

    pub fn output(&self, i: usize) -> &Output {
        &self.cache[i].output
    }

    /// Het aantal regio's met een fout.
    pub fn error_count(&self) -> usize {
        self.cache
            .iter()
            .filter(|c| matches!(c.output, Output::Error { .. }))
            .count()
    }

    /// Vervang één regel en herbereken alleen wat daardoor kan veranderen.
    /// Geeft de indices van de opnieuw berekende regio's terug (op volgorde).
    ///
    /// Verandert de positie van de regio (`@kolom,rij`), dan verandert de volgorde van doorrekenen;
    /// in dat geval wordt het hele blad opnieuw gesorteerd en doorgerekend.
    pub fn update_region(&mut self, idx: usize, new_line: &str) -> Vec<usize> {
        assert!(idx < self.regions.len(), "regio {idx} bestaat niet");
        let old = &self.regions[idx];
        let mut region = Region::from_line(old.line, new_line);
        if !region.placed {
            // zonder `@` blijft de regio waar hij stond
            region.pos = old.pos;
            region.placed = old.placed;
        }
        if region.pos != old.pos {
            self.regions[idx] = region;
            sort_regions(&mut self.regions);
            self.graph = Graph::build(&self.regions);
            return self.recalc_all();
        }
        let old_dependents = self.graph.dependents_closure(idx);
        let old_issues = self.graph.issues.clone();
        let old_blocks = self.solve_structure();

        self.regions[idx] = region;
        mark_given_blocks(&mut self.regions);
        self.graph = Graph::build(&self.regions);
        // een solve block veranderd (Given, beperking of afsluitende regel erbij of eraf): alles opnieuw
        if self.solve_structure() != old_blocks {
            return self.recalc_all();
        }

        let mut dirty = self.graph.dependents_closure(idx);
        dirty.extend(old_dependents);
        dirty.insert(idx);
        // regels waarvan de 'nog niet gedefinieerd'-status veranderde, moeten ook opnieuw
        for i in 0..self.graph.issues.len() {
            if old_issues.get(i) != Some(&self.graph.issues[i]) {
                dirty.insert(i);
                dirty.extend(self.graph.dependents_closure(i));
            }
        }
        self.recalc(Some(&dirty))
    }

    /// Per regio: is het `Given`, een beperking, en bij welke beperkingen hoort hij?
    fn solve_structure(&self) -> Vec<(bool, bool, Vec<usize>)> {
        self.regions
            .iter()
            .enumerate()
            .map(|(i, r)| {
                (
                    matches!(r.kind, Kind::Given),
                    matches!(r.kind, Kind::Constraint(_)),
                    self.graph.constraints[i].clone(),
                )
            })
            .collect()
    }

    fn recalc_all(&mut self) -> Vec<usize> {
        self.recalc(None)
    }

    /// Reken de regio's in `dirty` opnieuw uit (of alles bij `None`). Dit gebeurt op een thread met
    /// een ruime stack, zodat diep recursieve functies niet vastlopen.
    fn recalc(&mut self, dirty: Option<&BTreeSet<usize>>) -> Vec<usize> {
        with_stack(|| self.recalc_here(dirty))
    }

    fn recalc_here(&mut self, dirty: Option<&BTreeSet<usize>>) -> Vec<usize> {
        let mut env = Env::new();
        env.set_io(self.io.clone());
        let mut redone = Vec::new();
        for i in 0..self.regions.len() {
            if dirty.map_or(true, |d| d.contains(&i)) {
                // een oplosregel (Find, odesolve, ...) krijgt de beperkingen van zijn Given-blok
                let constraints = self.graph.constraints[i]
                    .iter()
                    .filter_map(|&j| match &self.regions[j].kind {
                        Kind::Constraint(e) => Some(e.clone()),
                        _ => None,
                    })
                    .collect();
                env.set_constraints(constraints);
                let c = self.eval_region(i, &env);
                self.cache[i] = c;
                redone.push(i);
            }
            for d in &self.cache[i].defined {
                env.apply(d.clone());
            }
        }
        redone
    }

    fn eval_region(&self, i: usize, env: &Env) -> Cached {
        if let Some(issue) = self.graph.issues[i].first() {
            return Cached::error(issue.message(), None);
        }
        for &j in &self.graph.deps[i] {
            if matches!(self.cache[j].output, Output::Error { .. }) {
                let name = self.graph.defines[j].clone().unwrap_or_default();
                return Cached::error(
                    format!(
                        "gebruikt '{name}' uit regel {}, en die bevat een fout",
                        self.regions[j].line
                    ),
                    None,
                );
            }
        }
        match &self.regions[i].kind {
            Kind::Heading(_) | Kind::Text(_) | Kind::Comment(_) | Kind::Given | Kind::Constraint(_) | Kind::Formula(_) | Kind::Image(_) => Cached::none(),
            Kind::Area { .. } | Kind::AreaEnd | Kind::PageBreak | Kind::HeaderFooter { .. } => Cached::none(),
            Kind::Invalid { err, caret } => {
                Cached::error(err.to_string(), if *caret { err.position() } else { None })
            }
            Kind::Math(stmt) => match env.eval_stmt(stmt) {
                // `a := ... = eenheid`: definiëren en meteen tonen
                Ok(Outcome::Defined(d)) if matches!(stmt, Stmt::Define { show: Some(_), .. }) => {
                    let Stmt::Define { show: Some(s), .. } = stmt else { unreachable!() };
                    match env.show_defined(&d, &s.unit).and_then(|(v, disp)| render_value(&v, &disp).map(|t| (t, v, disp))) {
                        Ok((t, v, disp)) => Cached {
                            output: Output::Value(t),
                            defined: vec![d],
                            result: Some((v, disp)),
                        },
                        Err(e) => Cached::error(e.to_string(), None),
                    }
                }
                Ok(Outcome::Defined(d)) => Cached {
                    output: Output::None,
                    defined: vec![d],
                    result: None,
                },
                Ok(Outcome::Value { value, display, .. }) => match render_value(&value, &display) {
                    Ok(t) => Cached {
                        output: Output::Value(t),
                        defined: Vec::new(),
                        result: Some((value, display)),
                    },
                    Err(e) => Cached::error(e.to_string(), None),
                },
                Ok(Outcome::Table { rows, .. }) => Cached {
                    output: Output::Table(rows),
                    defined: Vec::new(),
                    result: None,
                },
                Ok(Outcome::Symbolic { result, .. }) => Cached {
                    output: Output::Symbolic(result),
                    defined: Vec::new(),
                    result: None,
                },
                Err(e) => Cached::error(e.to_string(), None),
            },
            // een tabel: elke kolom is een definitie
            Kind::Table(t) => {
                let mut defined = Vec::new();
                for s in &t.stmts {
                    match env.eval_stmt(s) {
                        Ok(Outcome::Defined(d)) => defined.push(d),
                        Ok(_) => {}
                        Err(e) => return Cached::error(e.to_string(), None),
                    }
                }
                Cached { output: Output::None, defined, result: None }
            }
            Kind::Sweep(spec) => match eval_sweep(env, spec) {
                Ok(rows) => Cached {
                    output: Output::Table(rows),
                    defined: Vec::new(),
                    result: None,
                },
                Err(e) => Cached::error(e.to_string(), None),
            },
            Kind::Plot(spec) => match eval_plot(env, spec) {
                Ok(p) => Cached {
                    output: Output::Plot(p),
                    defined: Vec::new(),
                    result: None,
                },
                Err(e) => Cached::error(e.to_string(), None),
            },
        }
    }

    /// Het doorgerekende werkblad als tekst.
    pub fn render(&self) -> String {
        self.render_with(false)
    }

    /// Als `strict` waar is, worden ook de minder belangrijke waarschuwingen getoond
    /// (zoals "L is ook een eenheid").
    pub fn render_with(&self, strict: bool) -> String {
        let mut out = String::new();
        let mut prev_line = 0usize;
        for (i, r) in self.regions.iter().enumerate() {
            if prev_line != 0 && r.line > prev_line + 1 {
                out.push('\n');
            }
            // een programmablok beslaat meerdere regels
            prev_line = r.line + r.source.lines().count().saturating_sub(1);
            match (&r.kind, &self.cache[i].output) {
                (Kind::Heading(t) | Kind::Comment(t) | Kind::Text(t), _) => push(&mut out, t),
                (Kind::Image(img), _) => push(&mut out, &format!("[afbeelding {} × {} pt]", img.width, img.height)),
                (Kind::Math(Stmt::Eval { lhs_src, .. }), Output::Value(v)) => {
                    push(&mut out, &prefix_lines(&format!("{lhs_src} = "), v))
                }
                (Kind::Math(Stmt::Define { show: Some(s), .. }), Output::Value(v)) => {
                    push(&mut out, &prefix_lines(&format!("{} = ", s.head_src), v))
                }
                (Kind::Math(Stmt::Symbolic { lhs_src, .. }), Output::Symbolic(e)) => {
                    push(&mut out, &format!("{lhs_src} → {}", rb_core::symbolic::print(e)))
                }
                (_, Output::Plot(p)) => {
                    push(&mut out, &r.source);
                    push(&mut out, &format!("    grafiek: {}", plot_summary(p)));
                }
                (_, Output::Table(rows)) => {
                    push(&mut out, &r.source);
                    for row in rows {
                        push(&mut out, &prefix_lines("    ", row));
                    }
                }
                (_, Output::Error { message, col }) => {
                    push(&mut out, &r.source);
                    match col {
                        Some(byte_pos) => {
                            let chars = r.source.get(..*byte_pos).map_or(0, |s| s.chars().count());
                            push(&mut out, &format!("{}^ {}", " ".repeat(chars), message));
                        }
                        None => push(&mut out, &format!("  ✗ {message}")),
                    }
                }
                _ => push(&mut out, &r.source),
            }
            for w in &self.graph.warnings[i] {
                if !strict && matches!(w, Warning::ShadowsUnit { .. }) {
                    continue;
                }
                push(&mut out, &format!("  ⚠ {}", w.message()));
            }
        }
        out
    }

    /// Welke regio's gebruikt `name`, en wie gebruikt `name`? (Van de laatste definitie van die naam.)
    pub fn explain(&self, name: &str) -> Option<String> {
        let i = self
            .graph
            .defines
            .iter()
            .rposition(|d| d.as_deref() == Some(name))?;
        let list = |idxs: &[usize]| -> String {
            if idxs.is_empty() {
                return "    (geen)\n".to_string();
            }
            idxs.iter()
                .map(|&j| format!("    regel {}: {}\n", self.regions[j].line, self.regions[j].source))
                .collect()
        };
        let direct_deps: Vec<usize> = self.graph.deps[i].iter().copied().collect();
        let indirect_deps: Vec<usize> = self
            .graph
            .dependencies_closure(i)
            .into_iter()
            .filter(|j| !self.graph.deps[i].contains(j))
            .collect();
        let direct_users: Vec<usize> = self.graph.dependents[i].iter().copied().collect();
        let indirect_users: Vec<usize> = self
            .graph
            .dependents_closure(i)
            .into_iter()
            .filter(|j| !self.graph.dependents[i].contains(j))
            .collect();

        let mut s = format!(
            "{name} (regel {}): {}\n",
            self.regions[i].line, self.regions[i].source
        );
        s += "  hangt direct af van:\n";
        s += &list(&direct_deps);
        if !indirect_deps.is_empty() {
            s += "  en indirect van:\n";
            s += &list(&indirect_deps);
        }
        s += "  wordt direct gebruikt door:\n";
        s += &list(&direct_users);
        if !indirect_users.is_empty() {
            s += "  en beïnvloedt indirect:\n";
            s += &list(&indirect_users);
        }
        Some(s)
    }

    /// Overzicht van alle definities met hun gebruik; ongebruikte definities worden gemarkeerd.
    pub fn overview(&self) -> String {
        let mut s = String::new();
        for (i, def) in self.graph.defines.iter().enumerate() {
            if let Some(name) = def {
                let users = self.graph.dependents[i].len();
                let deps = self.graph.deps[i].len();
                let mark = if users == 0 { "  ← wordt nergens gebruikt" } else { "" };
                s += &format!(
                    "regel {:>3}  {:<12} gebruikt {} definitie(s), {} regel(s) gebruiken dit{}\n",
                    self.regions[i].line, name, deps, users, mark
                );
            }
        }
        if s.is_empty() {
            s.push_str("(geen definities)\n");
        }
        s
    }
}

fn push(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

fn eval_sweep(env: &Env, spec: &SweepSpec) -> Result<Vec<String>, Error> {
    let a = env.eval_expr(&spec.start)?.as_quantity("het begin van de sweep")?;
    let b = env.eval_expr(&spec.end)?.as_quantity("het einde van de sweep")?;
    let n = env
        .eval_expr(&spec.points)?
        .as_quantity("het aantal punten")?
        .dimensionless_value("het aantal punten")
        .map_err(Error::from)?;
    if n.fract() != 0.0 || !(2.0..=500.0).contains(&n) {
        return Err(Error::Domain(
            "sweep: het aantal punten moet een geheel getal van 2 tot en met 500 zijn".to_string(),
        ));
    }
    a.same_dim(b, "de sweep (begin en einde)").map_err(Error::from)?;
    let n = n as usize;

    let mut env2 = env.clone();
    let mut rows: Vec<(String, String)> = Vec::with_capacity(n);
    for k in 0..n {
        let t = k as f64 / (n - 1) as f64;
        let x = Quantity::new(a.value + (b.value - a.value) * t, a.dim);
        env2.set_var(&spec.var, x);
        let left = format!("{} = {}", spec.var, format_quantity(x));
        let right = match env2.eval_stmt(&spec.body) {
            Ok(Outcome::Value {
                value,
                lhs_src,
                display,
                ..
            }) => match render_value(&value, &display) {
                Ok(t) => prefix_lines(&format!("{lhs_src} = "), &t),
                Err(e) => format!("✗ {e}"),
            },
            Ok(Outcome::Table { .. }) => {
                "✗ een bereikvariabele in een sweep wordt niet ondersteund".to_string()
            }
            Ok(Outcome::Defined(_)) => "✗ geen resultaatregel".to_string(),
            Ok(Outcome::Symbolic { lhs_src, result }) => format!("{lhs_src} → {}", rb_core::symbolic::print(&result)),
            Err(e) => format!("✗ {e}"),
        };
        rows.push((left, right));
    }
    Ok(format_rows(rows))
}

/// Korte beschrijving van een grafiek voor de tekstweergave.
pub fn plot_summary(p: &PlotData) -> String {
    let n = p.series.first().map_or(0, |s| s.points.len());
    let xs: Vec<f64> = p.series.first().map(|s| s.points.iter().map(|q| q[0]).collect()).unwrap_or_default();
    let lo = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    format!(
        "{} reeks(en) × {n} punten, {} van {} tot {} (tekening: mathprime-cli export of de grafische versie)",
        p.series.len(),
        p.x_label,
        rb_units::format_number(lo),
        rb_units::format_number(hi)
    )
}
