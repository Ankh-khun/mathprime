//! Het lint: tabbladen met knoppen voor alles wat je ook met het toetsenbord kunt, zoals in Mathcad
//! Prime en Word. Een knop werkt op de formule die je bewerkt, of begint een nieuwe op het kruisje.

use crate::app::App;
use crate::canvas::{Background, Ins};
use crate::icons;
use crate::keymap::Action as MenuAction;
use eframe::egui::{self, RichText};
use rb_edit::keys::Sheet as S;
use rb_edit::tree::Op;
use rb_edit::{Cmd, Template as T};
use crate::taal::t as tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Wiskunde,
    Operatoren,
    Symbolen,
    Functies,
    Matrices,
    Programmeren,
    Eenheden,
    Berekenen,
    Document,
}

impl Tab {
    const ALL: [Tab; 9] = [
        Tab::Wiskunde,
        Tab::Operatoren,
        Tab::Symbolen,
        Tab::Functies,
        Tab::Matrices,
        Tab::Programmeren,
        Tab::Eenheden,
        Tab::Berekenen,
        Tab::Document,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Wiskunde => "Wiskunde",
            Tab::Operatoren => "Operatoren",
            Tab::Symbolen => "Symbolen",
            Tab::Functies => "Functies",
            Tab::Matrices => "Matrices",
            Tab::Programmeren => "Programmeren",
            Tab::Eenheden => "Eenheden",
            Tab::Berekenen => "Berekenen",
            Tab::Document => "Document",
        }
    }
}

/// Een knop: opschrift, uitleg (met sneltoets) en wat hij doet.
type B = (&'static str, &'static str, Ins);

fn t(x: T) -> Ins {
    Ins::Cmd(Cmd::Template(x))
}

fn c(ch: char) -> Ins {
    Ins::Cmd(Cmd::Char(ch))
}

fn groups(tab: Tab) -> Vec<(&'static str, Vec<B>)> {
    match tab {
        Tab::Wiskunde => vec![
            (
                "Regio's",
                vec![
                    ("Wiskunde", "Nieuwe wiskunderegio (Ctrl+Shift+M); of klik ergens en typ", Ins::Sheet(S::Math)),
                    ("Tekst", "Tekstregio (Ctrl+T of \")", Ins::Sheet(S::TextBox)),
                    ("Grafiek", "XY-grafiek (Ctrl+2)", Ins::Sheet(S::XyPlot)),
                    ("Solve block", "Given … Find (Ctrl+7)", Ins::Sheet(S::SolveBlock)),
                    ("Tabel", "Tabel met kolommen en eenheden (Ctrl+6)", Ins::Sheet(S::Table)),
                    ("Afbeelding", "PNG-afbeelding invoegen (of sleep een bestand naar het venster)", Ins::Image),
                ],
            ),
            (
                "Definities en uitkomsten",
                vec![
                    (":=", "Definitie (:)", c(':')),
                    ("=", "Uitrekenen (=); typ daarna een eenheid", c('=')),
                    ("←", "Lokale toekenning in een programma ({)", t(T::LocalAssign)),
                    ("1..n", "Bereik", Ins::Type("..")),
                ],
            ),
            (
                "Rekenen",
                vec![
                    ("x/y", "Breuk (/)", c('/')),
                    ("x^n", "Macht (^)", c('^')),
                    ("√x", "Wortel (\\)", c('\\')),
                    ("n√x", "n-de machtswortel (Ctrl+\\)", t(T::NthRoot)),
                    ("x[n]", "Index ([)", c('[')),
                    ("|x|", "Absolute waarde (|)", t(T::Abs)),
                    ("∑", "Som (Ctrl+Shift+4)", t(T::Sum)),
                    ("∫", "Integraal (Ctrl+Shift+I)", t(T::Integral)),
                    ("d/dx", "Afgeleide (Ctrl+Shift+D)", t(T::Deriv)),
                ],
            ),
            (
                "Bewerken",
                vec![
                    ("Ongedaan", "Ongedaan maken (Ctrl+Z)", Ins::Sheet(S::Undo)),
                    ("Opnieuw", "Opnieuw (Ctrl+Y)", Ins::Sheet(S::Redo)),
                    ("Knip", "Knippen (Ctrl+X)", Ins::Sheet(S::Cut)),
                    ("Kopie", "Kopiëren (Ctrl+C)", Ins::Sheet(S::Copy)),
                    ("αβ", "Letter voor de cursor ↔ Griekse letter (Ctrl+G)", Ins::Cmd(Cmd::Greek)),
                ],
            ),
        ],
        Tab::Operatoren => vec![
            (
                "Rekenkundig",
                vec![
                    ("+", "Optellen", c('+')),
                    ("−", "Aftrekken", c('-')),
                    ("·", "Vermenigvuldigen (*)", c('*')),
                    ("x/y", "Breuk (/)", c('/')),
                    ("x^n", "Macht (^)", c('^')),
                    ("√x", "Wortel (\\)", c('\\')),
                    ("n√x", "n-de machtswortel", t(T::NthRoot)),
                    ("|x|", "Absolute waarde (|)", t(T::Abs)),
                    ("n!", "Faculteit (!)", c('!')),
                    ("( )", "Haakjes", c('(')),
                ],
            ),
            (
                "Analyse",
                vec![
                    ("∑", "Som (Ctrl+Shift+4)", t(T::Sum)),
                    ("∏", "Product (Ctrl+Shift+3)", t(T::Prod)),
                    ("∫", "Integraal (Ctrl+Shift+I)", t(T::Integral)),
                    ("d/dx", "Afgeleide (Ctrl+Shift+D)", t(T::Deriv)),
                ],
            ),
            (
                "Vergelijken",
                vec![
                    ("=", "Is gelijk aan, voor voorwaarden en solve blocks (Ctrl+=)", t(T::Op(Op::BoolEq))),
                    ("<", "Kleiner dan", c('<')),
                    (">", "Groter dan", c('>')),
                    ("≤", "Kleiner of gelijk (Ctrl+9)", t(T::Op(Op::Le))),
                    ("≥", "Groter of gelijk (Ctrl+0)", t(T::Op(Op::Ge))),
                    ("≠", "Ongelijk (<>)", t(T::Op(Op::Ne))),
                ],
            ),
            (
                "Logisch",
                vec![
                    ("∧ and", "En (Ctrl+Shift+7)", t(T::Op(Op::And))),
                    ("∨ or", "Of (Ctrl+Shift+2)", t(T::Op(Op::Or))),
                    ("¬ not", "Niet (Ctrl+Shift+1)", t(T::Op(Op::Not))),
                ],
            ),
            (
                "Symbolisch",
                vec![
                    ("→", "Symbolisch uitrekenen (Ctrl+.)", c('→')),
                    ("simplify", "Vereenvoudigen", Ins::Type("→simplify")),
                    ("expand", "Uitwerken (haakjes wegwerken)", Ins::Type("→expand")),
                    ("factor", "Ontbinden in factoren", Ins::Type("→factor")),
                    ("solve", "Oplossen naar een variabele; typ daarna de naam", Ins::Type("→solve,")),
                    ("substitute", "Invullen: typ daarna x = waarde", Ins::Type("→substitute,")),
                    ("limit", "Limiet: typ daarna x = punt (ook ∞)", Ins::Type("→limit,")),
                    ("series", "Taylorreeks: typ daarna x = punt, orde", Ins::Type("→series,")),
                    ("float", "Als decimaal getal: typ daarna het aantal cijfers", Ins::Type("→float,")),
                ],
            ),
        ],
        Tab::Symbolen => {
            let greek: Vec<B> = GREEK.iter().map(|(g, name)| (*g, *name, Ins::Type(g))).collect();
            vec![
                ("Griekse letters (of letter + Ctrl+G)", greek),
                (
                    "Constanten",
                    vec![
                        ("π", "pi", Ins::Type("π")),
                        ("e", "getal van Euler", Ins::Type("e")),
                        ("∞", "oneindig (Ctrl+Shift+Z)", t(T::Infinity)),
                        ("i", "imaginaire eenheid (typ 1i)", Ins::Type("1i")),
                        ("°", "graden", Ins::Type("°")),
                        ("g", "valversnelling", Ins::Type("g")),
                    ],
                ),
            ]
        }
        Tab::Functies => FUNCTIONS
            .iter()
            .map(|(group, names)| {
                let bs: Vec<B> = names
                    .iter()
                    .filter(|n| rb_core::builtins::NAMES.contains(&n.trim_end_matches('(')))
                    .map(|n| (n.trim_end_matches('('), "Invoegen", Ins::Type(n)))
                    .collect();
                (*group, bs)
            })
            .chain(std::iter::once(("Alle functies", vec![("F2  Alle functies…", "Lijst met alle functies (F2)", Ins::Sheet(S::Functions))])))
            .collect(),
        Tab::Matrices => vec![
            (
                "Invoegen",
                vec![
                    ("Matrix", "Matrix 2×2 (Ctrl+M)", t(T::Matrix)),
                    ("+ rij", "Rij erbij (Shift+Enter)", Ins::Cmd(Cmd::AddRow)),
                    ("+ kolom", "Kolom erbij (Shift+Spatie)", Ins::Cmd(Cmd::AddColumn)),
                    ("M[i]", "Index ([)", c('[')),
                    ("M^-1", "Inverse", Ins::Type("^-1")),
                ],
            ),
            (
                "Functies",
                ["transpose(", "det(", "inverse(", "identity(", "lsolve(", "eigenvals(", "eigenvecs(", "rows(", "cols(", "cross(", "dot(", "norm(", "rank(", "augment(", "stack(", "submatrix("]
                    .iter()
                    .map(|n| (n.trim_end_matches('('), "Invoegen", Ins::Type(n)))
                    .collect(),
            ),
        ],
        Tab::Programmeren => vec![
            (
                "Programma",
                vec![
                    ("]  Programma", "Programmablok (])", t(T::Program)),
                    ("←", "Lokale toekenning ({)", t(T::LocalAssign)),
                    ("return", "Ctrl+\\", t(T::Return)),
                ],
            ),
            (
                "Voorwaarden",
                vec![
                    ("if", "}", t(T::If)),
                    ("else if", "Ctrl+;", t(T::ElseIf)),
                    ("else", "Ctrl+Shift+]", t(T::Else)),
                ],
            ),
            (
                "Lussen",
                vec![
                    ("for", "Ctrl+Shift+'", t(T::For)),
                    ("while", "Ctrl+]", t(T::While)),
                    ("break", "Ctrl+Shift+[", t(T::Break)),
                    ("continue", "Ctrl+Shift+;", t(T::Continue)),
                ],
            ),
        ],
        Tab::Eenheden => UNITS
            .iter()
            .map(|(group, units)| {
                let bs: Vec<B> = units.iter().map(|u| (*u, "Eenheid invoegen", Ins::Type(u))).collect();
                (*group, bs)
            })
            .collect(),
        Tab::Berekenen => vec![(
            "Berekenen",
            vec![
                ("Herberekenen", "F5", Ins::Sheet(S::Recalc)),
                ("Regio's scheiden", "Overlappende regio's uit elkaar (Ctrl+F3)", Ins::Sheet(S::SeparateVertical)),
            ],
        )],
        Tab::Document => vec![(
            "Pagina's",
            vec![
                ("Gebied", "Inklapbaar gebied om de selectie of op het kruisje (Ctrl+Shift+A)", Ins::Sheet(S::Area)),
                ("Pagina-einde", "Wat eronder staat, begint op een nieuwe pagina (Ctrl+Enter)", Ins::Sheet(S::PageBreak)),
                ("Koptekst", "Tekst bovenaan elke pagina; {pagina}, {paginas}, {bestand}, {datum}", Ins::Sheet(S::Header)),
                ("Voettekst", "Tekst onderaan elke pagina; {pagina}, {paginas}, {bestand}, {datum}", Ins::Sheet(S::Footer)),
            ],
        )],
    }
}

const GREEK: &[(&str, &str)] = &[
    ("α", "alpha (a)"),
    ("β", "beta (b)"),
    ("γ", "gamma (g)"),
    ("δ", "delta (d)"),
    ("ε", "epsilon (e)"),
    ("ζ", "zeta (z)"),
    ("η", "eta (h)"),
    ("θ", "theta (q)"),
    ("κ", "kappa (k)"),
    ("λ", "lambda (l)"),
    ("μ", "mu (m)"),
    ("ν", "nu (n)"),
    ("ξ", "xi (x)"),
    ("π", "pi (p)"),
    ("ρ", "rho (r)"),
    ("σ", "sigma (s)"),
    ("τ", "tau (t)"),
    ("φ", "phi (f)"),
    ("χ", "chi (c)"),
    ("ψ", "psi (y)"),
    ("ω", "omega (w)"),
    ("Γ", "Gamma (G)"),
    ("Δ", "Delta (D)"),
    ("Θ", "Theta (Q)"),
    ("Λ", "Lambda (L)"),
    ("Σ", "Sigma (S)"),
    ("Φ", "Phi (F)"),
    ("Ψ", "Psi (Y)"),
    ("Ω", "Omega (W)"),
];

const FUNCTIONS: &[(&str, &[&str])] = &[
    ("Goniometrie", &["sin(", "cos(", "tan(", "asin(", "acos(", "atan(", "atan2(", "sinh(", "cosh(", "tanh("]),
    ("Machten en logaritmen", &["sqrt(", "nthroot(", "exp(", "ln(", "log(", "log2("]),
    ("Afronden", &["round(", "floor(", "ceil(", "trunc(", "mod(", "abs(", "sign(", "min(", "max("]),
    ("Statistiek", &["mean(", "median(", "stdev(", "var(", "pnorm(", "qnorm(", "linfit(", "slope(", "intercept(", "regress(", "loess(", "ksmooth(", "medsmooth(", "localmax(", "localmin(", "filterNaN("]),
    ("Oplossen", &["root(", "polyroots(", "Find(", "Minerr(", "Minimize(", "Maximize(", "rkfixed(", "Radau(", "odesolve("]),
    ("Interpolatie", &["linterp(", "cspline(", "interp("]),
    ("Tekst", &["concat(", "strlen(", "substr(", "search(", "num2str(", "str2num("]),
    ("Bestanden (Excel, CSV)", &["READEXCEL(", "READCSV(", "READFILE(", "WRITEEXCEL(", "WRITECSV(", "WRITEFILE("]),
];

const UNITS: &[(&str, &[&str])] = &[
    ("Lengte", &["mm", "cm", "m", "km", "in", "ft"]),
    ("Kracht", &["N", "kN", "MN", "lbf"]),
    ("Spanning en druk", &["Pa", "kPa", "MPa", "GPa", "bar", "psi"]),
    ("Massa", &["g", "kg", "t", "lb"]),
    ("Tijd", &["s", "min", "hr", "day"]),
    ("Temperatuur", &["K", "°C", "°F"]),
    ("Energie en vermogen", &["J", "kJ", "W", "kW", "MW", "hp"]),
    ("Hoek", &["deg", "rad"]),
    ("Volume", &["L", "mL"]),
];

/// Hoe een knop eruitziet: (pictogram of symbool, opschrift). `None`: een gewone, compacte knop
/// (functienamen, eenheden, Griekse letters).
fn look(label: &str) -> Option<(String, String)> {
    let icon = |c: char, s: &str| Some((c.to_string(), s.to_string()));
    let sym = |s: &str| Some((label.to_string(), s.to_string()));
    match label {
        "Wiskunde" => icon(icons::FUNCTION, "Wiskunde"),
        "Tekst" => icon(icons::TEXT, "Tekst"),
        "Grafiek" => icon(icons::CHART, "Grafiek"),
        "Solve block" => icon(icons::TARGET, "Solve block"),
        "Tabel" => icon(icons::TABLE, "Tabel"),
        "Afbeelding" => icon(icons::PHOTO, "Afbeelding"),
        "Ongedaan" => icon(icons::UNDO, "Ongedaan"),
        "Opnieuw" => icon(icons::REDO, "Opnieuw"),
        "Knip" => icon(icons::CUT, "Knippen"),
        "Kopie" => icon(icons::COPY, "Kopiëren"),
        "Gebied" => icon(icons::AREA, "Gebied"),
        "Pagina-einde" => icon(icons::PAGE_BREAK, "Pagina-einde"),
        "Koptekst" => icon(icons::HEADER, "Koptekst"),
        "Voettekst" => icon(icons::FOOTER, "Voettekst"),
        ":=" => sym("Definitie"),
        "=" => sym("Uitrekenen"),
        "←" => sym("Lokaal"),
        "1..n" => sym("Bereik"),
        "x/y" => sym("Breuk"),
        "x^n" => sym("Macht"),
        "√x" => sym("Wortel"),
        "n√x" => sym("n-de wortel"),
        "x[n]" => sym("Index"),
        "|x|" => sym("Absoluut"),
        "∑" => sym("Som"),
        "∏" => sym("Product"),
        "∫" => sym("Integraal"),
        "d/dx" => sym("Afgeleide"),
        "+" => sym("Plus"),
        "−" => sym("Min"),
        "·" => sym("Maal"),
        "n!" => sym("Faculteit"),
        "( )" => sym("Haakjes"),
        "αβ" => sym("Grieks"),
        "<" | ">" | "≤" | "≥" | "≠" => sym(""),
        "∧ and" => Some(("∧".into(), "En".into())),
        "∨ or" => Some(("∨".into(), "Of".into())),
        "¬ not" => Some(("¬".into(), "Niet".into())),
        "→" => sym("Symbolisch"),
        "simplify" | "expand" | "factor" | "solve" | "substitute" | "limit" | "series" | "float" => Some(("→".into(), label.to_string())),
        _ => None,
    }
}

impl App {
    pub(crate) fn ribbon(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            ui.add_space(4.0);
            for tab in Tab::ALL {
                if icons::underline_tab(ui, tr(tab.label()), self.tab == tab).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.add_space(6.0);
        let mut chosen: Option<Ins> = None;
        egui::ScrollArea::horizontal().id_salt("lint").show(ui, |ui| {
            ui.horizontal(|ui| {
                if self.tab == Tab::Document {
                    self.document_tab(ctx, ui);
                }
                if self.tab == Tab::Wiskunde {
                    self.formula_group(ui, &mut chosen);
                }
                for (title, buttons) in groups(self.tab) {
                    if buttons.is_empty() {
                        continue;
                    }
                    // knoppen met een pictogram of een bekend symbool groot, met een opschrift;
                    // lange lijsten (functies, eenheden, Griekse letters) compact in rijen
                    let big = buttons.iter().all(|(label, ..)| look(label).is_some());
                    icons::ribbon_group(ui, tr(title), |ui| {
                        if big {
                            for (label, tip, ins) in &buttons {
                                let (top, caption) = look(label).unwrap_or_default();
                                if icons::ribbon_button(ui, &top, tr(&caption), false).on_hover_text(tr(tip)).clicked() {
                                    chosen = Some(ins.clone());
                                }
                            }
                        } else {
                            ui.vertical(|ui| {
                                for chunk in buttons.chunks(10) {
                                    ui.horizontal(|ui| {
                                        for (label, tip, ins) in chunk {
                                            if ui.add(egui::Button::new(RichText::new(*label).size(14.0)).frame_when_inactive(false)).on_hover_text(tr(tip)).clicked() {
                                                chosen = Some(ins.clone());
                                            }
                                        }
                                    });
                                }
                            });
                        }
                    });
                }
            });
        });
        if let Some(ins) = chosen {
            self.insert(ctx, ins);
        }
    }

    /// Bibliotheek en omwerken: de grootheden van de formule waar je in staat.
    fn formula_group(&mut self, ui: &mut egui::Ui, chosen: &mut Option<Ins>) {
        let vars = self.current_equation().map(|(l, r)| rb_core::solve::unknowns(&l, &r)).unwrap_or_default();
        icons::ribbon_group(ui, tr("Formules"), |ui| {
            if icons::ribbon_button(ui, &icons::BOOK.to_string(), tr("Bibliotheek"), self.lib.show)
                .on_hover_text(tr("Bibliotheek met formules (en je eigen formules)"))
                .clicked()
            {
                *chosen = Some(Ins::Library);
            }
            let resp = icons::ribbon_button(ui, &icons::REFRESH.to_string(), tr("Omwerken ▾"), false).on_hover_text(if vars.is_empty() {
                "Klik eerst op een formule, zoals F == m * a of F := m * a"
            } else {
                "De formule waar je in staat oplossen naar een andere grootheid"
            });
            egui::Popup::menu(&resp).show(|ui| {
                if vars.is_empty() {
                    ui.label(RichText::new(tr("Klik eerst op een formule")).weak());
                }
                for v in &vars {
                    if ui.button(v).clicked() {
                        *chosen = Some(Ins::Solve(v.clone()));
                        ui.close();
                    }
                }
            });
        });
    }

    fn document_tab(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        use crate::settings::Thema;
        icons::ribbon_group(ui, tr("Thema"), |ui| {
            for (t, icon, label) in [(Thema::Licht, icons::SUN, "Licht"), (Thema::Donker, icons::MOON, "Donker"), (Thema::Systeem, icons::DESKTOP, "Windows")] {
                if icons::ribbon_button(ui, &icon.to_string(), label, self.cv.set.thema == t).clicked() {
                    self.cv.set.thema = t;
                    self.apply_theme(ctx);
                    self.save_settings();
                }
            }
        });
        icons::ribbon_group(ui, tr("Achtergrond van de pagina"), |ui| {
            for b in Background::ALL {
                let icon = match b {
                    Background::Leeg => icons::EMPTY,
                    Background::Raster => icons::DOTS,
                    Background::Ruitjes => icons::GRID,
                    Background::Lijnen => icons::LINES,
                };
                if icons::ribbon_button(ui, &icon.to_string(), tr(b.label()), self.cv.set.background == b).clicked() {
                    self.cv.set.background = b;
                    self.save_settings();
                }
            }
        });
        icons::ribbon_group(ui, tr("Uitvoer"), |ui| {
            if icons::ribbon_button(ui, &icons::PDF.to_string(), "PDF", false).on_hover_text(tr("Exporteren naar PDF (Ctrl+Shift+P)")).clicked() {
                self.run(ctx, MenuAction::ExportPdf);
            }
            if icons::ribbon_button(ui, &icons::VECTOR.to_string(), "SVG", false).on_hover_text(tr("Exporteren naar SVG")).clicked() {
                self.run(ctx, MenuAction::ExportSvg);
            }
            if icons::ribbon_button(ui, &icons::EXPORT.to_string(), "Mathcad", false).on_hover_text(tr("Opslaan als Mathcad Prime-bestand (.mcdx)")).clicked() {
                self.export_mcdx();
            }
            if icons::ribbon_button(ui, &icons::CODE.to_string(), "Bron", false).on_hover_text(tr("De tekst van het werkblad tonen of verbergen (Ctrl+Shift+B)")).clicked() {
                self.run(ctx, MenuAction::BronTonen);
            }
        });
    }
}
