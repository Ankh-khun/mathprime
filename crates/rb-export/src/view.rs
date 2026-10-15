//! Wat een regio laat zien, los van hoe het getekend wordt (interface, SVG of PDF).

use rb_core::Stmt;
use rb_layout::{expr_box, stmt_box, value_box, LayoutOptions, MBox, Metrics};
use rb_sheet::{Kind, Output, PlotData, Sheet, Warning};

/// De inhoud van één regio.
#[derive(Debug, Clone)]
pub enum Content {
    /// Kop (`# ...`); `level` is het aantal `#`.
    Heading { text: String, level: usize },
    /// Gewone tekst (`" ...`).
    Text(String),
    /// Commentaar (`// ...`).
    Comment(String),
    /// Een opgemaakte formule.
    Formula(MBox),
    /// Rijen van een sweep of van een resultaat over een bereik, met de formule of bron erboven.
    Table { head: MBox, rows: Vec<String> },
    Plot { source: String, data: PlotData },
    /// Een afbeelding.
    Image(rb_sheet::Image),
    /// Een tabel: kolomnamen, eenheden en de cellen per rij.
    Grid { names: Vec<String>, units: Vec<String>, rows: Vec<Vec<String>> },
    /// Begin van een gebied: een balk over de breedte met ▾/▸ en de titel.
    AreaStart { title: String, collapsed: bool },
    /// Einde van een gebied: een lijn.
    AreaEnd,
    /// Pagina-einde (op het scherm een stippellijn, in PDF onzichtbaar).
    PageBreak,
    /// Kop- of voettekst (op het scherm als regio; in PDF op elke pagina).
    HeaderFooter { text: String, footer: bool },
    /// Een regel die niet gelezen kon worden: de bron als tekst.
    Source(String),
}

#[derive(Debug, Clone)]
pub struct View {
    pub content: Content,
    /// Foutmelding onder de regio.
    pub error: Option<String>,
    pub warnings: Vec<String>,
}

/// Maak de weergave van regio `i`.
pub fn region_view(sheet: &Sheet, i: usize, m: &dyn Metrics, opts: LayoutOptions, strict: bool) -> View {
    let r = &sheet.regions()[i];
    let out = sheet.output(i);
    let error = match out {
        Output::Error { message, .. } => Some(message.clone()),
        _ => None,
    };
    let warnings = sheet.graph().warnings[i]
        .iter()
        .filter(|w| strict || !matches!(w, Warning::ShadowsUnit { .. }))
        .map(|w| w.message())
        .collect();
    let source_head = |s: &str| rb_layout_text(m, s, opts);
    let content = match (&r.kind, out) {
        (Kind::Heading(t), _) => {
            let level = t.chars().take_while(|c| *c == '#').count();
            Content::Heading {
                text: t[level..].trim().to_string(),
                level,
            }
        }
        (Kind::Text(t), _) => Content::Text(t.clone()),
        (Kind::Comment(t), _) => Content::Comment(t.trim_start_matches('/').trim().to_string()),
        (Kind::Given, _) => Content::Formula(keyword(m, "Given", opts)),
        (Kind::Constraint(e) | Kind::Formula(e), _) => Content::Formula(expr_box(m, e, opts)),
        (Kind::Math(stmt), Output::Table(rows)) => Content::Table {
            head: stmt_box(m, stmt, None, opts),
            rows: rows.clone(),
        },
        (Kind::Math(stmt), Output::Symbolic(e)) => Content::Formula(stmt_box(m, stmt, Some(expr_box(m, e, opts)), opts)),
        (Kind::Math(stmt), _) => {
            let result = sheet.result(i).map(|(v, d)| value_box(m, v, d, opts));
            let is_eval = matches!(stmt, Stmt::Eval { .. } | Stmt::Define { show: Some(_), .. });
            Content::Formula(stmt_box(m, stmt, if is_eval { result } else { None }, opts))
        }
        (Kind::Sweep(_), Output::Table(rows)) => Content::Table {
            head: source_head(&r.source),
            rows: rows.clone(),
        },
        (Kind::Plot(_), Output::Plot(data)) => Content::Plot {
            source: r.source.clone(),
            data: data.clone(),
        },
        (Kind::Image(img), _) => Content::Image(img.clone()),
        (Kind::Area { title, collapsed }, _) => Content::AreaStart { title: title.clone(), collapsed: *collapsed },
        (Kind::AreaEnd, _) => Content::AreaEnd,
        (Kind::PageBreak, _) => Content::PageBreak,
        (Kind::HeaderFooter { text, footer }, _) => Content::HeaderFooter { text: text.clone(), footer: *footer },
        (Kind::Table(t), _) => Content::Grid {
            names: t.cols.iter().map(|c| c.name.clone()).collect(),
            units: t.cols.iter().map(|c| c.unit.clone()).collect(),
            rows: t.rows.clone(),
        },
        _ => Content::Source(r.source.clone()),
    };
    View { content, error, warnings }
}

fn rb_layout_text(m: &dyn Metrics, s: &str, opts: LayoutOptions) -> MBox {
    MBox {
        width: m.width(s, opts.size),
        ascent: m.ascent() * opts.size,
        descent: m.descent() * opts.size,
        items: vec![rb_layout::Item::Text {
            x: 0.0,
            y: 0.0,
            size: opts.size,
            text: s.to_string(),
            style: rb_layout::Style::Normal,
        }],
    }
}

fn keyword(m: &dyn Metrics, s: &str, opts: LayoutOptions) -> MBox {
    let mut b = rb_layout_text(m, s, opts);
    if let Some(rb_layout::Item::Text { style, .. }) = b.items.first_mut() {
        *style = rb_layout::Style::Keyword;
    }
    b
}
