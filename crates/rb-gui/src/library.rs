//! De formulebibliotheek: veelgebruikte formules (``formules.txt``) en eigen formules (``formules.txt`` in de
//! instellingenmap). Een formule invoegen kan als uitleg (alleen tonen) of omgewerkt naar de grootheid
//! die je zoekt, met de andere grootheden als invoer.

use rb_core::solve::{expr_sides, isolate, sides, unknowns};
use rb_core::{parse_expr_str, parse_stmt, Expr, Show, Stmt};
use std::collections::BTreeSet;
use std::path::Path;

const BUILTIN: &str = include_str!("formules.txt");

#[derive(Debug, Clone, PartialEq)]
pub struct Var {
    pub name: String,
    pub what: String,
    /// Voorbeeldeenheid voor de invoer (leeg of `1`: dimensieloos).
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Formula {
    pub category: String,
    pub name: String,
    /// De vergelijking, zoals `F == m * a`.
    pub eq: String,
    pub vars: Vec<Var>,
    /// Een eigen formule (kan verwijderd worden).
    pub own: bool,
}

impl Formula {
    /// Alle grootheden: eerst die met een omschrijving, dan de rest uit de vergelijking (niet `g`, `π`).
    pub fn quantities(&self) -> Vec<String> {
        let mut out: Vec<String> = self.vars.iter().map(|v| v.name.clone()).collect();
        if let Some((l, r)) = parse_expr_str(&self.eq).ok().as_ref().and_then(expr_sides) {
            for n in unknowns(&l, &r) {
                if !out.contains(&n) && !matches!(n.as_str(), "g" | "π" | "e") {
                    out.push(n);
                }
            }
        }
        out
    }

    pub fn var(&self, name: &str) -> Option<&Var> {
        self.vars.iter().find(|v| v.name == name)
    }

    /// Als regel voor het bestand.
    pub fn to_line(&self) -> String {
        let vars: Vec<String> = self
            .vars
            .iter()
            .map(|v| if v.unit.is_empty() { format!("{}: {}", v.name, v.what) } else { format!("{}: {} [{}]", v.name, v.what, v.unit) })
            .collect();
        format!("{} | {} | {}", self.name, self.eq, vars.join("; "))
    }
}

/// Lees een bibliotheekbestand.
pub fn parse(text: &str, own: bool) -> Vec<Formula> {
    let mut category = if own { "Eigen formules".to_string() } else { String::new() };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(c) = line.strip_prefix("## ") {
            category = c.trim().to_string();
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        if parts.len() < 2 {
            continue;
        }
        let vars = parts
            .get(2)
            .map(|v| {
                v.split(';')
                    .filter_map(|d| {
                        let (name, rest) = d.split_once(':')?;
                        let (what, unit) = match rest.rsplit_once('[') {
                            Some((w, u)) => (w.trim(), u.trim_end_matches(']').trim()),
                            None => (rest.trim(), ""),
                        };
                        Some(Var { name: name.trim().to_string(), what: what.to_string(), unit: unit.to_string() })
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(Formula { category: category.clone(), name: parts[0].to_string(), eq: parts[1].to_string(), vars, own });
    }
    out
}

pub fn builtin() -> Vec<Formula> {
    parse(BUILTIN, false)
}

pub fn load_own(dir: &Path) -> Vec<Formula> {
    std::fs::read_to_string(dir.join("formules.txt")).map(|t| parse(&t, true)).unwrap_or_default()
}

/// Schrijf de eigen formules (per categorie).
pub fn save_own(dir: &Path, own: &[Formula]) -> std::io::Result<()> {
    let mut text = String::from("# Eigen formules voor MathPrime: naam | vergelijking | grootheid: omschrijving [eenheid]; ...\n");
    let mut cats: Vec<&str> = own.iter().map(|f| f.category.as_str()).collect();
    cats.dedup();
    let mut seen = BTreeSet::new();
    for c in cats {
        if !seen.insert(c) {
            continue;
        }
        text.push_str(&format!("\n## {c}\n"));
        for f in own.iter().filter(|f| f.category == c) {
            text.push_str(&f.to_line());
            text.push('\n');
        }
    }
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("formules.txt"), text)
}

/// De vergelijking van een regio (`a == b`, of een definitie `a := b`), als die er een is.
pub fn equation_of(source: &str) -> Option<(Expr, Expr)> {
    let code = source.split("//").next().unwrap_or(source).trim();
    if let Ok(stmt) = parse_stmt(code, false) {
        return sides(&stmt);
    }
    parse_expr_str(code).ok().as_ref().and_then(expr_sides)
}

/// De tekst van een regel `naam := uitdrukking = eenheid`.
fn define_text(name: &str, body: Expr, unit: &str) -> String {
    let unit = if unit.is_empty() || unit == "1" { None } else { parse_expr_str(unit).ok().map(|u| (u, unit.to_string())) };
    let stmt = Stmt::Define { name: name.to_string(), params: None, index: None, body, show: Some(Show { unit, head_src: String::new() }) };
    rb_edit::region_text(&rb_edit::from_stmt(&stmt))
}

fn value_text(unit: &str) -> String {
    if unit.is_empty() || unit == "1" {
        "1".to_string()
    } else if let Some(per) = unit.strip_prefix("1/") {
        // 1 per minuut: `1/min`
        format!("1/{per}")
    } else if unit.starts_with(|c: char| c.is_ascii_digit()) {
        format!("1 * ({unit})")
    } else {
        format!("1 {unit}")
    }
}

/// De regels om `lhs == rhs` op te lossen naar `target`: omgewerkt als dat kan, anders met een solve
/// block. `inputs`: de andere grootheden (met hun voorbeeldeenheid en omschrijving) die nog niet
/// gedefinieerd zijn, krijgen een invoerregel met waarde 1.
pub fn solve_lines(lhs: &Expr, rhs: &Expr, target: &str, unit_of: &dyn Fn(&str) -> (String, String), inputs: &[String]) -> Vec<String> {
    let mut lines = Vec::new();
    // namen die ook een eenheid zijn (m, s, A, ...) als laatste: anders zou `m := 1 kg` de meter in de
    // invoer daarna overschaduwen
    let mut rest = inputs.to_vec();
    let mut inputs = Vec::new();
    let uses_unit = |unit: &str, name: &str| unit.split(|c: char| !c.is_alphanumeric() && c != 'µ' && c != '°').any(|t| t == name);
    while !rest.is_empty() {
        // eerst een naam die geen enkele overgebleven invoer als eenheid gebruikt
        let k = (0..rest.len())
            .find(|&k| !rest.iter().enumerate().any(|(j, o)| j != k && uses_unit(&unit_of(o).0, &rest[k])))
            .unwrap_or(0);
        inputs.push(rest.remove(k));
    }
    for n in &inputs {
        let (unit, what) = unit_of(n);
        let comment = if what.is_empty() { String::new() } else { format!(" // {what}") };
        lines.push(format!("{n} := {}{comment}", value_text(&unit)));
    }
    let (unit, _) = unit_of(target);
    match isolate(lhs, rhs, target) {
        Some(body) => lines.push(define_text(target, body, &unit)),
        None => {
            // numeriek, zoals in Mathcad: beginwaarde, Given, de vergelijking, Find

            let l = rb_edit::convert::expr_text(lhs);
            let r = rb_edit::convert::expr_text(rhs);
            // de beginwaarde vóór de invoer, want een invoernaam kan een eenheid overschaduwen (s, m)
            lines.insert(0, format!("{target} := {} // beginwaarde", value_text(&unit)));
            lines.push("Given".into());
            lines.push(format!("{l} == {r}"));
            let show = if unit.is_empty() || unit == "1" { String::new() } else { format!(" {unit}") };
            lines.push(format!("{target} := Find({target}) ={show}"));
        }
    }
    lines
}

/// De regels voor een formule uit de bibliotheek, opgelost naar `target`. `defined`: namen die het
/// blad al kent (die krijgen geen invoerregel).
pub fn formula_lines(f: &Formula, target: &str, with_inputs: bool, defined: &BTreeSet<String>) -> Vec<String> {
    let Some((l, r)) = parse_expr_str(&f.eq).ok().as_ref().and_then(expr_sides) else { return vec![format!("// {}", f.eq)] };
    let unit_of = |n: &str| f.var(n).map_or((String::new(), String::new()), |v| (v.unit.clone(), v.what.clone()));
    let inputs: Vec<String> = if with_inputs {
        f.quantities().into_iter().filter(|n| n != target && !defined.contains(n)).collect()
    } else {
        Vec::new()
    };
    solve_lines(&l, &r, target, &unit_of, &inputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bibliotheek_is_leesbaar() {
        let all = builtin();
        assert!(all.len() > 60, "{}", all.len());
        for f in &all {
            assert!(parse_expr_str(&f.eq).ok().as_ref().and_then(expr_sides).is_some(), "{}: {}", f.name, f.eq);
            assert!(!f.category.is_empty());
            // elke grootheid in de vergelijking heeft een omschrijving
            for q in f.quantities() {
                assert!(f.var(&q).is_some(), "{}: '{q}' zonder omschrijving", f.name);
            }
        }
    }

    /// Elke formule, opgelost naar elke grootheid (met invoerwaarden), rekent foutloos door, met
    /// eenheden.
    #[test]
    fn elke_formule_naar_elke_grootheid() {
        for f in builtin() {
            for q in f.quantities() {
                let lines = formula_lines(&f, &q, true, &BTreeSet::new());
                // verschillende invoerwaarden (met allemaal 1 deelt 1 - η door nul)
                let src: Vec<String> = lines
                    .iter()
                    .enumerate()
                    .map(|(k, l)| match l.split_once(" := 1") {
                        Some((n, rest)) if l.contains("//") => format!("{n} := {}{rest}", 0.31 + 0.17 * k as f64),
                        _ => l.clone(),
                    })
                    .collect();
                let src = src.join("\n");
                let sheet = rb_sheet::Sheet::load(&src);
                // bij willekeurige invoer heeft een numerieke oplossing soms geen oplossing; de rest moet kloppen
                let errors = (0..sheet.regions().len())
                    .filter(|&i| matches!(sheet.output(i), rb_sheet::Output::Error { message, .. } if !message.starts_with("Find: geen oplossing")))
                    .count();
                assert_eq!(errors, 0, "{} naar {q}:\n{src}\n---\n{}", f.name, sheet.render());
            }
        }
    }

    #[test]
    fn eigen_formules_heen_en_terug() {
        let dir = std::env::temp_dir().join(format!("mathprime-test-{}", std::process::id()));
        let own = vec![Formula {
            category: "Mijn vak".into(),
            name: "Vermogen pomp".into(),
            eq: "P == Q * Δp".into(),
            vars: vec![Var { name: "P".into(), what: "vermogen".into(), unit: "kW".into() }],
            own: true,
        }];
        save_own(&dir, &own).unwrap();
        assert_eq!(load_own(&dir), own);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn omwerken_vanuit_een_regio() {
        let (l, r) = equation_of("σ := M / W").unwrap();
        let none = |_: &str| (String::new(), String::new());
        assert_eq!(solve_lines(&l, &r, "W", &none, &[]), vec!["W := M/σ ="]);
        let (l, r) = equation_of("x == cos(x)").unwrap();
        let lines = solve_lines(&l, &r, "x", &none, &[]);
        assert_eq!(lines[1], "Given");
        assert!(rb_sheet::Sheet::load(&lines.join("\n")).render().contains("x := Find(x) = 0.739085"));
    }
}

// ---------------------------------------------------------------------------------------------
// Het paneel

use crate::app::App;
use eframe::egui::{self, RichText};
use crate::taal::t;

/// Wat het paneel bijhoudt.
pub struct LibraryState {
    pub show: bool,
    builtin: Vec<Formula>,
    own: Vec<Formula>,
    search: String,
    with_inputs: bool,
    form: Option<Form>,
}

impl Default for LibraryState {
    fn default() -> Self {
        LibraryState { show: false, builtin: builtin(), own: Vec::new(), search: String::new(), with_inputs: true, form: None }
    }
}

struct Form {
    name: String,
    category: String,
    eq: String,
    vars: String,
    error: Option<String>,
}

enum Act {
    Solve(Formula, String),
    Show(Formula),
    Delete(Formula),
}

impl App {
    fn library_dir(&self) -> Option<std::path::PathBuf> {
        Some(self.keymap.path.as_ref()?.parent()?.to_path_buf())
    }

    pub(crate) fn load_library(&mut self) {
        if let Some(d) = self.library_dir() {
            self.lib.own = load_own(&d);
        }
    }

    /// Namen die het blad al definieert.
    fn defined_names(&self) -> BTreeSet<String> {
        self.sheet.graph().defines.iter().flatten().cloned().collect()
    }

    pub(crate) fn library_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Formules");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Sluiten").clicked() {
                    self.lib.show = false;
                }
            });
        });
        ui.add(egui::TextEdit::singleline(&mut self.lib.search).hint_text(t("Zoeken (naam of grootheid)…")).desired_width(f32::INFINITY));
        ui.checkbox(&mut self.lib.with_inputs, t("Met invoerwaarden (waarde 1, pas ze aan)"))
            .on_hover_text(t("Grootheden die het blad nog niet kent, krijgen een regel met waarde 1 en een eenheid"));
        if ui.button(t("+ Eigen formule opslaan…")).clicked() {
            let eq = self.current_equation().map(|(l, r)| format!("{} == {}", rb_edit::convert::expr_text(&l), rb_edit::convert::expr_text(&r))).unwrap_or_default();
            self.lib.form = Some(Form { name: String::new(), category: "Eigen formules".into(), eq, vars: String::new(), error: None });
        }
        ui.separator();
        let q = self.lib.search.to_lowercase();
        let all: Vec<Formula> = self.lib.own.iter().chain(&self.lib.builtin).cloned().collect();
        let matches = |f: &Formula| {
            q.is_empty()
                || f.name.to_lowercase().contains(&q)
                || f.category.to_lowercase().contains(&q)
                || f.eq.to_lowercase().contains(&q)
                || f.vars.iter().any(|v| v.what.to_lowercase().contains(&q))
        };
        let mut cats: Vec<String> = Vec::new();
        for f in all.iter().filter(|f| matches(f)) {
            if !cats.contains(&f.category) {
                cats.push(f.category.clone());
            }
        }
        let mut act = None;
        let dark = self.cv.dark;
        egui::ScrollArea::vertical().id_salt("bibliotheek").show(ui, |ui| {
            for cat in &cats {
                egui::CollapsingHeader::new(RichText::new(cat).strong()).default_open(!q.is_empty()).show(ui, |ui| {
                    for f in all.iter().filter(|f| &f.category == cat && matches(f)) {
                        ui.group(|ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&f.name).strong());
                                if f.own && ui.small_button("🗑").on_hover_text(t("Uit de bibliotheek verwijderen")).clicked() {
                                    act = Some(Act::Delete(f.clone()));
                                }
                            });
                            formula_view(ui, &f.eq, dark);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(t("Oplossen naar:")).small());
                                for name in f.quantities() {
                                    let tip = f.var(&name).map_or(String::new(), |v| if v.unit.is_empty() { v.what.clone() } else { format!("{} [{}]", v.what, v.unit) });
                                    if ui.button(&name).on_hover_text(tip).clicked() {
                                        act = Some(Act::Solve(f.clone(), name));
                                    }
                                }
                                if ui.small_button("als uitleg").on_hover_text(t("De formule alleen tonen (niet uitrekenen)")).clicked() {
                                    act = Some(Act::Show(f.clone()));
                                }
                            });
                        });
                    }
                });
            }
            if cats.is_empty() {
                ui.label(t("Niets gevonden."));
            }
        });
        match act {
            Some(Act::Solve(f, name)) => {
                let lines = formula_lines(&f, &name, self.lib.with_inputs, &self.defined_names());
                self.insert_lines(ctx, lines);
                self.status = format!("{}: opgelost naar {name}", f.name);
            }
            Some(Act::Show(f)) => self.insert_lines(ctx, vec![f.eq.clone()]),
            Some(Act::Delete(f)) => {
                self.lib.own.retain(|o| o != &f);
                self.save_library();
            }
            None => {}
        }
    }

    fn save_library(&mut self) {
        if let Some(d) = self.library_dir() {
            if let Err(e) = save_own(&d, &self.lib.own) {
                self.status = format!("Kan de eigen formules niet opslaan: {e}");
            }
        }
    }

    /// Venster om een eigen formule op te slaan.
    pub(crate) fn library_form(&mut self, ctx: &egui::Context) {
        let Some(form) = &mut self.lib.form else { return };
        let mut open = true;
        let mut save = false;
        egui::Window::new(t("Eigen formule opslaan")).open(&mut open).collapsible(false).default_width(420.0).show(ctx, |ui| {
            egui::Grid::new("formulier").num_columns(2).show(ui, |ui| {
                ui.label(t("Naam"));
                ui.text_edit_singleline(&mut form.name);
                ui.end_row();
                ui.label(t("Categorie"));
                ui.text_edit_singleline(&mut form.category);
                ui.end_row();
                ui.label(t("Vergelijking"));
                ui.add(egui::TextEdit::singleline(&mut form.eq).hint_text(t("F == m * a")).font(egui::TextStyle::Monospace));
                ui.end_row();
                ui.label(t("Grootheden"));
                ui.add(egui::TextEdit::multiline(&mut form.vars).hint_text("F: kracht [N]\nm: massa [kg]\na: versnelling [m/s^2]").desired_rows(4));
                ui.end_row();
            });
            ui.label(RichText::new(t("Per grootheid: naam: omschrijving [eenheid]. Optioneel; de eenheid wordt gebruikt voor de invoerwaarden.")).small());
            if let Some(e) = &form.error {
                ui.colored_label(egui::Color32::from_rgb(190, 30, 30), e);
            }
            if ui.button(t("Opslaan")).clicked() {
                save = true;
            }
        });
        if save {
            let form = self.lib.form.as_mut().expect("formulier");
            let ok = parse_expr_str(&form.eq).ok().as_ref().and_then(expr_sides).is_some();
            if form.name.trim().is_empty() {
                form.error = Some("Geef de formule een naam.".into());
            } else if !ok {
                form.error = Some("De vergelijking moet de vorm 'links == rechts' hebben (Ctrl+= op het werkblad).".into());
            } else {
                let vars_text = form.vars.lines().collect::<Vec<_>>().join("; ");
                let line = format!("{} | {} | {}", form.name.trim(), form.eq.trim(), vars_text);
                let mut f = parse(&line, true).pop().expect("formule");
                f.category = if form.category.trim().is_empty() { "Eigen formules".into() } else { form.category.trim().to_string() };
                self.status = format!("Opgeslagen in de bibliotheek: {}", f.name);
                self.lib.own.push(f);
                self.save_library();
                self.lib.form = None;
                self.lib.show = true;
                return;
            }
        }
        if !open {
            self.lib.form = None;
        }
    }
}

/// Een formule opgemaakt tonen (zoals op het werkblad).
fn formula_view(ui: &mut egui::Ui, eq: &str, dark: bool) {
    let Ok(e) = parse_expr_str(eq) else {
        ui.monospace(eq);
        return;
    };
    let b = rb_layout::expr_box(&rb_export::FontMetrics, &e, rb_layout::LayoutOptions { size: 13.0 });
    let zoom = 1.25;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(b.width * zoom + 4.0, b.height() * zoom + 4.0), egui::Sense::hover());
    let mut ops = Vec::new();
    rb_export::doc::box_ops(&b, 0.0, b.ascent, &mut ops);
    crate::paint::paint_ops(ui.painter(), &ops, rect.min + egui::vec2(2.0, 2.0), zoom, dark);
}
