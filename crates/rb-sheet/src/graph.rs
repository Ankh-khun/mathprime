//! Afhankelijkheidsgraaf van een werkblad.
//!
//! Net als in Mathcad geldt: een naam moet *boven* de plek van gebruik gedefinieerd zijn. Een regel
//! hangt af van de laatste definitie van elke naam die hij gebruikt, die boven hem staat.

use crate::parse::{Kind, Region, SOLVERS};
use rb_core::{builtins, constant_value};
use rb_units::{lookup_offset_unit, lookup_unit};
use std::collections::{BTreeSet, HashMap};

/// Problemen die de regel onuitvoerbaar maken.
#[derive(Debug, Clone, PartialEq)]
pub enum Issue {
    /// De naam wordt gebruikt voordat hij gedefinieerd is (wel verderop in het blad).
    UsedBeforeDefined { name: String, defined_line: usize },
}

impl Issue {
    pub fn message(&self) -> String {
        match self {
            Issue::UsedBeforeDefined { name, defined_line } => format!(
                "'{name}' is hier nog niet gedefinieerd (wel op regel {defined_line}, verderop in het blad). \
                 Zet de definitie boven deze regel"
            ),
        }
    }
}

/// Waarschuwingen: de regel werkt, maar is waarschijnlijk niet wat je bedoelt.
#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    Redefined { name: String, previous_line: usize },
    /// De naam is ook een eenheid; vanaf hier is het in dit blad een variabele (alleen met `--strikt`).
    ShadowsUnit { name: String },
    /// De naam wordt hier gebruikt als eenheid/constante/functie, maar verderop als variabele gedefinieerd.
    UnitBeforeVariable { name: String, defined_line: usize },
}

impl Warning {
    pub fn message(&self) -> String {
        match self {
            Warning::Redefined { name, previous_line } => {
                format!("'{name}' was al gedefinieerd op regel {previous_line}; vanaf hier geldt de nieuwe waarde")
            }
            Warning::ShadowsUnit { name } => format!(
                "'{name}' is ook een eenheid; in dit blad is het vanaf hier een variabele"
            ),
            Warning::UnitBeforeVariable { name, defined_line } => format!(
                "'{name}' geldt hier als eenheid/constante/functie; pas op regel {defined_line} wordt het een variabele"
            ),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Graph {
    /// Welke naam definieert elke regio (indien van toepassing).
    pub defines: Vec<Option<String>>,
    /// Meer namen van dezelfde regio (de andere kolommen van een tabel).
    pub also_defines: Vec<Vec<String>>,
    /// `deps[i]`: regio's waarvan regio `i` direct afhangt.
    pub deps: Vec<BTreeSet<usize>>,
    /// `dependents[j]`: regio's die direct van regio `j` afhangen.
    pub dependents: Vec<BTreeSet<usize>>,
    pub issues: Vec<Vec<Issue>>,
    pub warnings: Vec<Vec<Warning>>,
    /// `constraints[i]`: de beperkingen van het Given-blok dat regio `i` afsluit (Find, odesolve, ...).
    pub constraints: Vec<Vec<usize>>,
}

impl Graph {
    pub fn build(regions: &[Region]) -> Graph {
        let n = regions.len();
        let mut g = Graph {
            defines: vec![None; n],
            also_defines: vec![Vec::new(); n],
            deps: vec![BTreeSet::new(); n],
            dependents: vec![BTreeSet::new(); n],
            issues: vec![Vec::new(); n],
            warnings: vec![Vec::new(); n],
            constraints: vec![Vec::new(); n],
        };
        // solve blocks: welke beperkingen horen bij welke afsluitende regio, en welke naam wordt
        // daar gedefinieerd (bij `y := odesolve(...)` mogen de beperkingen y, y', y'' gebruiken)
        let mut exempt: Vec<Option<String>> = vec![None; n];
        let mut block: Option<Vec<usize>> = None;
        for (i, r) in regions.iter().enumerate() {
            match &r.kind {
                Kind::Given => block = Some(Vec::new()),
                Kind::Constraint(_) => {
                    if let Some(b) = &mut block {
                        b.push(i);
                    }
                }
                Kind::Math(stmt) if block.is_some() => {
                    let closes = stmt.uses().iter().any(|u| SOLVERS.contains(&u.as_str()))
                        || matches!(stmt, rb_core::Stmt::Define { body: rb_core::Expr::Bind { op: rb_core::BindOp::OdeSolve, .. }, .. });
                    if closes {
                        let b = block.take().unwrap();
                        for &j in &b {
                            exempt[j] = stmt.defines().map(str::to_string);
                        }
                        g.constraints[i] = b;
                    }
                }
                _ => {}
            }
        }
        let mut uses: Vec<BTreeSet<String>> = vec![BTreeSet::new(); n];
        let mut is_fn = vec![false; n];
        // `v[i] := ...` vult een bestaande vector aan en hangt dus af van de vorige definitie
        let mut extends = vec![false; n];
        let mut all_defs: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, r) in regions.iter().enumerate() {
            match &r.kind {
                Kind::Math(stmt) => {
                    uses[i] = stmt.uses();
                    if let Some(name) = stmt.defines() {
                        g.defines[i] = Some(name.to_string());
                        is_fn[i] = stmt.is_function();
                        extends[i] = stmt.updates_existing();
                        all_defs.entry(name.to_string()).or_default().push(i);
                    }
                }
                Kind::Table(t) => {
                    uses[i] = t.uses();
                    let names = t.defines();
                    for name in &names {
                        all_defs.entry(name.clone()).or_default().push(i);
                    }
                    g.defines[i] = names.first().cloned();
                    g.also_defines[i] = names[1..].to_vec();
                }
                Kind::Sweep(spec) => uses[i] = spec.uses(),
                Kind::Plot(spec) => uses[i] = spec.uses(),
                Kind::Constraint(e) => {
                    uses[i] = e
                        .names()
                        .into_iter()
                        .filter(|u| exempt[i].as_deref() != Some(u.trim_end_matches('\'')))
                        .collect()
                }
                _ => {}
            }
        }

        let mut latest: HashMap<String, usize> = HashMap::new();
        for i in 0..n {
            for &j in &g.constraints[i].clone() {
                g.deps[i].insert(j);
                g.dependents[j].insert(i);
            }
            for name in &uses[i] {
                // een functie mag zichzelf aanroepen
                if is_fn[i] && g.defines[i].as_deref() == Some(name.as_str()) {
                    continue;
                }
                if let Some(&j) = latest.get(name) {
                    g.deps[i].insert(j);
                    g.dependents[j].insert(i);
                } else if let Some(list) = all_defs.get(name) {
                    if let Some(&j) = list.iter().find(|&&j| j > i) {
                        if is_known(name) {
                            // zoals in Mathcad: bovenaan is `m` gewoon de meter, ook als `m`
                            // verderop een variabele wordt. Wel een waarschuwing.
                            g.warnings[i].push(Warning::UnitBeforeVariable {
                                name: name.clone(),
                                defined_line: regions[j].line,
                            });
                        } else {
                            g.issues[i].push(Issue::UsedBeforeDefined {
                                name: name.clone(),
                                defined_line: regions[j].line,
                            });
                        }
                    }
                }
            }
            let names: Vec<String> = g.defines[i].iter().chain(&g.also_defines[i]).cloned().collect();
            for name in names {
                if let Some(&prev) = latest.get(&name) {
                    if extends[i] {
                        g.deps[i].insert(prev);
                        g.dependents[prev].insert(i);
                    } else {
                        g.warnings[i].push(Warning::Redefined {
                            name: name.clone(),
                            previous_line: regions[prev].line,
                        });
                    }
                }
                if lookup_unit(&name).is_some() && !extends[i] {
                    g.warnings[i].push(Warning::ShadowsUnit { name: name.clone() });
                }
                latest.insert(name, i);
            }
        }
        g
    }

    /// Alle regio's die (direct of indirect) van regio `i` afhangen.
    pub fn dependents_closure(&self, i: usize) -> BTreeSet<usize> {
        closure(&self.dependents, i)
    }

    /// Alle regio's waarvan regio `i` (direct of indirect) afhangt.
    pub fn dependencies_closure(&self, i: usize) -> BTreeSet<usize> {
        closure(&self.deps, i)
    }
}

/// Is dit een naam die ook zonder definitie bestaat (eenheid, constante of ingebouwde functie)?
fn is_known(name: &str) -> bool {
    lookup_unit(name).is_some()
        || lookup_offset_unit(name).is_some()
        || constant_value(name).is_some()
        || builtins::NAMES.contains(&name)
}

fn closure(edges: &[BTreeSet<usize>], start: usize) -> BTreeSet<usize> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![start];
    while let Some(x) = stack.pop() {
        if x >= edges.len() {
            continue;
        }
        for &d in &edges[x] {
            if seen.insert(d) {
                stack.push(d);
            }
        }
    }
    seen
}
