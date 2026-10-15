//! De sneltoetsen van Mathcad Prime 10 (Engelse toetsenbordindeling), vertaald naar commando's.
//!
//! Bron: `Doc/en-US/keyboard_shortcuts.pdf` van de Prime-installatie. Toetsen waarvan de functie in
//! MathPrime (nog) niet bestaat, geven [`Action::Unsupported`] met een korte uitleg, zodat je weet
//! waarom er niets gebeurt.

use crate::tree::{Cmd, Op, Template};

/// Een toets, los van de interface. Letters als hoofdletter, cijfers en leestekens zoals op de
/// toets zelf (zonder Shift), dus Ctrl+Shift+4 is `Char('4')` met `shift`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Backspace,
    Delete,
    Enter,
    Space,
    Escape,
    Tab,
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl KeyPress {
    pub fn new(key: Key) -> KeyPress {
        KeyPress { key, ctrl: false, shift: false, alt: false }
    }
    pub fn ctrl(key: Key) -> KeyPress {
        KeyPress { ctrl: true, ..KeyPress::new(key) }
    }
    pub fn ctrl_shift(key: Key) -> KeyPress {
        KeyPress { ctrl: true, shift: true, ..KeyPress::new(key) }
    }
}

/// Acties op het niveau van het werkblad (buiten de formule zelf).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sheet {
    TextBox,
    TextBlock,
    Math,
    XyPlot,
    SolveBlock,
    Table,
    PageBreak,
    Area,
    Header,
    Footer,
    Recalc,
    RecalcAll,
    SeparateVertical,
    SeparateHorizontal,
    Help,
    Functions,
    New,
    Open,
    Save,
    Print,
    Close,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Edit(Cmd),
    Sheet(Sheet),
    Unsupported(&'static str),
}

/// Vertaal een toets naar een actie. `in_math`: de cursor staat in een formule (sommige toetsen
/// betekenen dan iets anders, zoals Ctrl+Shift+T: transponeren in een formule, tekstblok erbuiten).
/// Gewone tekens (zonder Ctrl) komen niet hier langs maar als getypte tekst ([`Cmd::Char`]).
pub fn mathcad_key(k: KeyPress, in_math: bool) -> Option<Action> {
    use Action::{Edit, Sheet as S, Unsupported as U};
    use Key::*;
    let t = |t: Template| Some(Edit(Cmd::Template(t)));
    let (c, s, a) = (k.ctrl, k.shift, k.alt);
    Some(match (k.key, c, s, a) {
        // --- navigeren en bewerken
        (Left, false, false, _) => Edit(Cmd::Left),
        (Right, false, false, _) => Edit(Cmd::Right),
        (Up, false, false, _) => Edit(Cmd::Up),
        (Down, false, false, _) => Edit(Cmd::Down),
        (Left, true, _, _) => Edit(Cmd::Left),
        (Right, true, _, _) => Edit(Cmd::Right),
        (Home, _, _, _) => Edit(Cmd::Home),
        (End, _, _, _) => Edit(Cmd::End),
        (Backspace, _, _, _) => Edit(Cmd::Backspace),
        (Delete, false, _, _) => Edit(Cmd::Delete),
        (Escape, _, _, _) => Edit(Cmd::Escape),
        (Tab, _, _, _) => Edit(Cmd::Right),
        (Space, false, false, _) => Edit(Cmd::Grow),
        (Space, false, true, _) => Edit(Cmd::AddColumn),
        (Enter, false, true, _) => Edit(Cmd::AddRow),
        (Enter, true, true, _) => U("vergelijkingsafbreking (Ctrl+Shift+Enter) wordt nog niet ondersteund"),
        (Enter, true, false, _) => S(Sheet::PageBreak),
        (Enter, false, false, _) => Edit(Cmd::Enter),

        // --- regio's
        (Char('T'), true, false, false) => S(Sheet::TextBox),
        (Char('T'), true, true, false) if !in_math => S(Sheet::TextBlock),
        (Char('T'), true, true, false) => U("transponeren met Ctrl+Shift+T komt nog; schrijf transpose(M)"),
        (Char('M'), true, true, false) => S(Sheet::Math),
        (Char('2'), true, false, false) => S(Sheet::XyPlot),
        (Char('7'), true, false, false) => S(Sheet::SolveBlock),
        (Char('3'), true, false, false) => U("3D-grafieken worden nog niet ondersteund"),
        (Char('4'), true, false, false) => U("poolgrafieken worden nog niet ondersteund"),
        (Char('5'), true, false, false) => U("contourgrafieken worden nog niet ondersteund"),
        (Char('6'), true, false, false) => S(Sheet::Table),
        (Char('1'), true, false, false) => U("grafiekcomponenten worden nog niet ondersteund"),
        (Char('A'), true, true, false) => S(Sheet::Area),
        (Char('E'), true, true, false) => U("Excel-componenten worden niet ondersteund"),
        (Char('W'), true, true, false) => U("werkbladen invoegen (include) wordt nog niet ondersteund"),
        (Char('O'), true, true, false) => U("objecten invoegen wordt niet ondersteund"),
        (F(5), true, _, _) => S(Sheet::RecalcAll),
        (F(5), false, _, _) => S(Sheet::Recalc),
        (F(3), true, _, _) => S(Sheet::SeparateVertical),
        (F(8), true, _, _) => S(Sheet::SeparateHorizontal),
        (F(1), _, _, _) => S(Sheet::Help),
        (F(2), _, _, _) => S(Sheet::Functions),

        // --- algemeen
        (Char('N'), true, false, false) => S(Sheet::New),
        (Char('O'), true, false, false) => S(Sheet::Open),
        (Char('S'), true, false, false) => S(Sheet::Save),
        (Char('P'), true, false, false) => S(Sheet::Print),
        (Char('W'), true, false, false) => S(Sheet::Close),
        (Char('Z'), true, false, false) => S(Sheet::Undo),
        (Char('Y'), true, false, false) => S(Sheet::Redo),
        (Char('X'), true, false, false) => S(Sheet::Cut),
        (Char('C'), true, false, false) => S(Sheet::Copy),
        (Char('V'), true, false, false) => S(Sheet::Paste),
        (Char('K'), true, false, false) => U("koppelingen worden nog niet ondersteund"),
        (Char('G'), true, false, true) => U("ga naar pagina komt met de paginaweergave"),
        (Char('B' | 'I' | 'U'), true, false, false) => U("tekstopmaak (vet, cursief, onderstreept) komt nog"),
        (Char('Q'), true, false, false) => U("labels wisselen wordt niet ondersteund"),

        // --- formules: tekens en constanten
        (Char('G'), true, false, false) => Edit(Cmd::Greek),
        (Char('Z'), true, true, false) => t(Template::Infinity)?,
        (Char('-'), true, false, false) => Edit(Cmd::Char('.')),
        (Char('='), true, false, false) => t(Template::Op(Op::BoolEq))?,
        (Char('0'), true, false, false) => t(Template::Op(Op::Ge))?,
        (Char('9'), true, false, false) => t(Template::Op(Op::Le))?,
        (F(7), true, _, _) => Edit(Cmd::Char('∈')),
        (Char('\''), true, false, false) => Edit(Cmd::Char('\'')),
        (Char('/'), true, false, false) => Edit(Cmd::Char('/')),

        // --- rekenoperatoren
        (Char('D'), true, true, false) => t(Template::Deriv)?,
        (Char('D'), true, false, false) => U("partiële afgeleiden worden nog niet ondersteund; gebruik d/dx (Ctrl+Shift+D)"),
        (Char('I'), true, true, false) => t(Template::Integral)?,
        (Char('4'), true, true, false) => t(Template::Sum)?,
        (Char('3'), true, true, false) => t(Template::Prod)?,
        (Char('V' | 'L'), true, true, false) => U("convoluties worden nog niet ondersteund"),
        (Char('G'), true, true, false) => U("de gradiëntoperator wordt nog niet ondersteund"),
        (Char('L'), true, false, false) => U("limieten (symbolisch) worden niet ondersteund"),
        (Char('.'), true, false, false) => Edit(Cmd::Char('→')),

        // --- logica
        (Char('7'), true, true, false) => t(Template::Op(Op::And))?,
        (Char('2'), true, true, false) => t(Template::Op(Op::Or))?,
        (Char('1'), true, true, false) => t(Template::Op(Op::Not))?,
        (Char('5'), true, true, false) => U("exclusieve of (xor) wordt nog niet ondersteund"),

        // --- vectoren en matrices
        (Char('M'), true, false, false) => t(Template::Matrix)?,
        (Char('8'), true, false, false) => U("kruisproduct als operator komt nog; schrijf cross(a, b)"),
        (Char('C' | 'R'), true, true, false) => U("matrixkolom en -rij als operator komen nog; schrijf submatrix(...)"),
        (Char('\\'), true, true, false) => U("de norm als operator komt nog; schrijf norm(M)"),
        (Char('6'), true, true, false) => U("vectoriseren is niet nodig: functies werken in MathPrime al element voor element"),

        // --- programmeren
        (Char(';'), true, false, false) => t(Template::ElseIf)?,
        (Char(']'), true, true, false) => t(Template::Else)?,
        (Char(']'), true, false, false) => t(Template::While)?,
        (Char('\''), true, true, false) => t(Template::For)?,
        (Char('['), true, true, false) => t(Template::Break)?,
        (Char(';'), true, true, false) => t(Template::Continue)?,
        (Char('\\'), true, false, false) => t(Template::Return)?,
        (Char('['), true, false, false) => U("try / on error wordt nog niet ondersteund"),
        (Char('0'), true, true, false) => U("'also if' wordt nog niet ondersteund"),
        (Char('J'), true, false, false) => U("Ctrl+J (programmawoord als operator) is niet nodig: typ for, while, ... met de sneltoetsen"),

        // --- engineering
        (Char('9'), true, true, false) => U("complex toevoegen als operator komt nog; schrijf conj(z)"),
        (Char('U'), true, true, false) => U("schaling wordt nog niet ondersteund"),
        (Char('P'), true, true, false) => U("polaire notatie wordt nog niet ondersteund"),
        (Char('`'), true, true, false) => U("globale definities (≡) worden nog niet ondersteund"),
        (Char('='), true, true, false) | (Char('-'), true, true, false) | (Char('8'), true, true, false) | (Char('/'), true, true, false) => {
            U("vergelijkingsafbreking wordt nog niet ondersteund")
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sneltoetsen_uit_de_prime_documentatie() {
        let e = |k| mathcad_key(k, true);
        assert_eq!(e(KeyPress::ctrl_shift(Key::Char('D'))), Some(Action::Edit(Cmd::Template(Template::Deriv))));
        assert_eq!(e(KeyPress::ctrl_shift(Key::Char('4'))), Some(Action::Edit(Cmd::Template(Template::Sum))));
        assert_eq!(e(KeyPress::ctrl(Key::Char('M'))), Some(Action::Edit(Cmd::Template(Template::Matrix))));
        assert_eq!(e(KeyPress::ctrl(Key::Char(']'))), Some(Action::Edit(Cmd::Template(Template::While))));
        assert_eq!(e(KeyPress::ctrl_shift(Key::Char(']'))), Some(Action::Edit(Cmd::Template(Template::Else))));
        assert_eq!(e(KeyPress::ctrl(Key::Char('G'))), Some(Action::Edit(Cmd::Greek)));
        assert_eq!(e(KeyPress::ctrl(Key::Char('2'))), Some(Action::Sheet(Sheet::XyPlot)));
        assert_eq!(mathcad_key(KeyPress::ctrl_shift(Key::Char('T')), false), Some(Action::Sheet(Sheet::TextBlock)));
        assert!(matches!(e(KeyPress::ctrl_shift(Key::Char('T'))), Some(Action::Unsupported(_))));
        assert!(matches!(e(KeyPress::ctrl(Key::Char('.'))), Some(Action::Edit(Cmd::Char('→')))));
        assert_eq!(e(KeyPress::new(Key::Space)), Some(Action::Edit(Cmd::Grow)));
        assert_eq!(e(KeyPress { shift: true, ..KeyPress::new(Key::Enter) }), Some(Action::Edit(Cmd::AddRow)));
    }
}
