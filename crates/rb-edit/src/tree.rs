//! De bewerkbare formule: een boom van blokken (rijen atomen) met een cursor, zoals in Mathcad Prime.
//!
//! Een [`Block`] is een rij [`Atom`]s; sommige atomen (breuk, macht, wortel, haakjes, matrix, Σ, ...)
//! bevatten zelf weer blokken: hun *plaatsen*. Een leeg blok is een placeholder (het zwarte vierkantje
//! in Mathcad). De cursor wijst een positie aan tussen twee atomen in één blok.

/// Operatoren met een vaste weergave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `:=`
    Define,
    /// `=` (uitrekenen)
    Eval,
    Plus,
    Minus,
    /// `·`
    Times,
    Comma,
    Lt,
    Gt,
    Le,
    Ge,
    Ne,
    /// vetgedrukte `=` (vergelijken, Ctrl+=)
    BoolEq,
    And,
    Or,
    Not,
    /// `←` (lokale toekenning in een programma)
    Assign,
    /// `!`
    Factorial,
    /// `→` (symbolisch rekenen, Ctrl+.)
    Arrow,
}

impl Op {
    pub fn symbol(self) -> &'static str {
        match self {
            Op::Define => ":=",
            Op::Eval => "=",
            Op::Plus => "+",
            Op::Minus => "−",
            Op::Times => "·",
            Op::Comma => ",",
            Op::Lt => "<",
            Op::Gt => ">",
            Op::Le => "≤",
            Op::Ge => "≥",
            Op::Ne => "≠",
            Op::BoolEq => "=",
            Op::And => "∧",
            Op::Or => "∨",
            Op::Not => "¬",
            Op::Assign => "←",
            Op::Factorial => "!",
            Op::Arrow => "→",
        }
    }
}

/// Soort groot teken of sjabloon met vaste plaatsen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Big {
    /// Σ: plaatsen [variabele, van, tot, uitdrukking]
    Sum,
    /// ∏: plaatsen [variabele, van, tot, uitdrukking]
    Prod,
    /// ∫: plaatsen [ondergrens, bovengrens, uitdrukking, variabele]
    Integral,
    /// d/dx: plaatsen [variabele, uitdrukking, orde]
    Deriv,
}

/// Programmeeroperatoren (met een kop en een ingesprongen programma eronder).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ctrl {
    /// kop: [voorwaarde]
    If,
    /// kop: [voorwaarde]
    ElseIf,
    /// kop: []
    Else,
    /// kop: [voorwaarde]
    While,
    /// kop: [variabele, bereik]
    For,
}

impl Ctrl {
    pub fn keyword(self) -> &'static str {
        match self {
            Ctrl::If => "if",
            Ctrl::ElseIf => "else if",
            Ctrl::Else => "else",
            Ctrl::While => "while",
            Ctrl::For => "for",
        }
    }

    pub fn head_len(self) -> usize {
        match self {
            Ctrl::Else => 0,
            Ctrl::For => 2,
            _ => 1,
        }
    }
}

/// Sleutelwoorden zonder plaatsen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kw {
    Break,
    Continue,
    /// `return`, gevolgd door de rest van de regel
    Return,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Block {
    pub atoms: Vec<Atom>,
}

impl Block {
    pub fn new(atoms: Vec<Atom>) -> Block {
        Block { atoms }
    }

    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    pub fn len(&self) -> usize {
        self.atoms.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    /// Een teken van een naam of getal (letters, cijfers, `.`, `'`, Griekse letters, `∞`, `°`).
    Char(char),
    Op(Op),
    Frac { num: Block, den: Block },
    /// Macht van het voorgaande.
    Pow(Block),
    /// Index `[ ]` van het voorgaande (vector- of matrixelement).
    Index(Block),
    Sqrt(Block),
    Root { index: Block, radicand: Block },
    Paren(Block),
    Abs(Block),
    Matrix { rows: usize, cols: usize, cells: Vec<Block> },
    Big { kind: Big, slots: Vec<Block> },
    /// Een programma: één blok per regel.
    Program(Vec<Block>),
    Ctrl { kind: Ctrl, head: Vec<Block>, body: Vec<Block> },
    Kw(Kw),
    /// Tekst tussen aanhalingstekens (`"abc"`); de inhoud zijn losse tekens.
    Str(Block),
}

impl Atom {
    /// De plaatsen (deelblokken) in de volgorde waarin de cursor ze doorloopt.
    pub fn slots(&self) -> Vec<&Block> {
        match self {
            Atom::Char(_) | Atom::Op(_) | Atom::Kw(_) => vec![],
            Atom::Frac { num, den } => vec![num, den],
            Atom::Pow(b) | Atom::Index(b) | Atom::Sqrt(b) | Atom::Paren(b) | Atom::Abs(b) | Atom::Str(b) => vec![b],
            Atom::Root { index, radicand } => vec![index, radicand],
            Atom::Matrix { cells, .. } => cells.iter().collect(),
            Atom::Big { slots, .. } => slots.iter().collect(),
            Atom::Program(lines) => lines.iter().collect(),
            Atom::Ctrl { head, body, .. } => head.iter().chain(body.iter()).collect(),
        }
    }

    pub fn slots_mut(&mut self) -> Vec<&mut Block> {
        match self {
            Atom::Char(_) | Atom::Op(_) | Atom::Kw(_) => vec![],
            Atom::Frac { num, den } => vec![num, den],
            Atom::Pow(b) | Atom::Index(b) | Atom::Sqrt(b) | Atom::Paren(b) | Atom::Abs(b) | Atom::Str(b) => vec![b],
            Atom::Root { index, radicand } => vec![index, radicand],
            Atom::Matrix { cells, .. } => cells.iter_mut().collect(),
            Atom::Big { slots, .. } => slots.iter_mut().collect(),
            Atom::Program(lines) => lines.iter_mut().collect(),
            Atom::Ctrl { head, body, .. } => head.iter_mut().chain(body.iter_mut()).collect(),
        }
    }

    pub fn is_structure(&self) -> bool {
        !self.slots().is_empty()
    }

    /// Hoort dit atoom bij een operand (naam, getal, haakjes, breuk, ...), niet bij een operator?
    fn is_operand(&self) -> bool {
        !matches!(self, Atom::Op(_) | Atom::Kw(_) | Atom::Ctrl { .. } | Atom::Program(_))
    }
}

/// Een cursorpositie: het pad van atoomindex en plaatsnummer vanaf de wortel, en de positie in
/// het blok aan het eind van dat pad.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cursor {
    pub path: Vec<(usize, usize)>,
    pub pos: usize,
}

/// Het effect van een toets op de regio als geheel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Gewoon doorgaan met bewerken.
    None,
    /// De regio is af (Enter, `=` of Escape): opslaan en uitrekenen.
    Commit,
    /// `"` in een lege regio: maak er een tekstregio van.
    ToText,
    /// De toets doet (nog) niets; met uitleg voor de statusbalk.
    Unsupported(&'static str),
}

/// Sjablonen die met sneltoetsen worden ingevoegd.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Sum,
    Prod,
    Integral,
    Deriv,
    Matrix,
    Program,
    If,
    ElseIf,
    Else,
    While,
    For,
    Break,
    Continue,
    Return,
    LocalAssign,
    Op(Op),
    Infinity,
    NthRoot,
    Abs,
}

/// Een bewerkingscommando (onafhankelijk van welke toets het gaf).
#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    /// Een getypt teken.
    Char(char),
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Backspace,
    Delete,
    /// Spatie: de selectie vergroten.
    Grow,
    /// Enter
    Enter,
    /// Shift+Enter: rij toevoegen (matrix) of regel (programma)
    AddRow,
    /// Shift+Spatie: kolom toevoegen (matrix)
    AddColumn,
    Escape,
    /// Ctrl+G: letter voor de cursor ↔ Griekse letter
    Greek,
    Template(Template),
}

/// De formule-editor van één regio.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Editor {
    pub root: Block,
    pub cursor: Cursor,
    /// Geselecteerd bereik `[van, tot)` in het blok van de cursor.
    pub selection: Option<(usize, usize)>,
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '\'' | '°' | '∞' | '.')
}

impl Editor {
    pub fn new() -> Editor {
        Editor::default()
    }

    /// Een editor met deze inhoud en de cursor aan het eind.
    pub fn with_root(root: Block) -> Editor {
        let pos = root.len();
        Editor { root, cursor: Cursor { path: vec![], pos }, selection: None }
    }

    pub fn block(&self, path: &[(usize, usize)]) -> &Block {
        let mut b = &self.root;
        for &(i, s) in path {
            b = b.atoms[i].slots()[s];
        }
        b
    }

    pub fn block_mut(&mut self, path: &[(usize, usize)]) -> &mut Block {
        let mut b = &mut self.root;
        for &(i, s) in path {
            b = b.atoms[i].slots_mut().into_iter().nth(s).expect("geldig pad");
        }
        b
    }

    fn cur(&self) -> &Block {
        self.block(&self.cursor.path)
    }

    fn cur_mut(&mut self) -> &mut Block {
        let p = self.cursor.path.clone();
        self.block_mut(&p)
    }

    /// Het atoom waar het huidige blok een plaats van is (de ouder), als dat er is.
    fn parent_atom(&self) -> Option<&Atom> {
        let (last, rest) = self.cursor.path.split_last()?;
        Some(&self.block(rest).atoms[last.0])
    }

    fn insert(&mut self, a: Atom) {
        self.delete_selection();
        let pos = self.cursor.pos;
        self.cur_mut().atoms.insert(pos, a);
        self.cursor.pos += 1;
    }

    fn delete_selection(&mut self) -> bool {
        if let Some((a, b)) = self.selection.take() {
            self.cur_mut().atoms.drain(a..b);
            self.cursor.pos = a;
            true
        } else {
            false
        }
    }

    /// Ga naar plaats `slot` van het atoom op index `i` in het huidige blok (aan het begin of eind).
    fn enter(&mut self, i: usize, slot: usize, at_end: bool) {
        self.cursor.path.push((i, slot));
        self.cursor.pos = if at_end { self.cur().len() } else { 0 };
    }

    /// Verlaat het huidige blok: de cursor komt voor of na het ouderatoom.
    fn exit(&mut self, after: bool) -> bool {
        match self.cursor.path.pop() {
            Some((i, _)) => {
                self.cursor.pos = if after { i + 1 } else { i };
                true
            }
            None => false,
        }
    }

    /// Begin van de operand die eindigt op `end` in het huidige blok. `chain`: ook over `·` en
    /// breuken heen (voor `/`, zoals `q·L^2/8`).
    fn operand_start(&self, end: usize, chain: bool) -> usize {
        let atoms = &self.cur().atoms;
        let mut k = end;
        if !chain {
            // de laatste naam of het laatste getal, of het laatste structuuratoom (met zijn macht/index)
            while k > 0 && matches!(atoms[k - 1], Atom::Pow(_) | Atom::Index(_) | Atom::Op(Op::Factorial)) {
                k -= 1;
            }
            if k > 0 {
                match &atoms[k - 1] {
                    Atom::Char(c) if is_name_char(*c) => {
                        // een naam begint bij een letter; een getal is een reeks cijfers en punten
                        let mut j = k;
                        while j > 0 && matches!(atoms[j - 1], Atom::Char(c) if is_name_char(c)) {
                            j -= 1;
                        }
                        // bij `2x` hoort alleen `x` bij de macht
                        let run: Vec<char> = atoms[j..k].iter().map(|a| if let Atom::Char(c) = a { *c } else { ' ' }).collect();
                        if let Some(first_letter) = run.iter().position(|c| !c.is_ascii_digit() && *c != '.') {
                            if first_letter > 0 && run[..first_letter].iter().all(|c| c.is_ascii_digit() || *c == '.') {
                                j += first_letter;
                            }
                        }
                        k = j;
                    }
                    a if a.is_operand() => k -= 1,
                    _ => {}
                }
            }
            return k;
        }
        while k > 0 {
            match &atoms[k - 1] {
                Atom::Op(Op::Times) => k -= 1,
                a if a.is_operand() => k -= 1,
                _ => break,
            }
        }
        k
    }

    /// Het bereik waarop een operator werkt: de selectie, of de operand voor de cursor.
    fn take_operand(&mut self, chain: bool) -> Vec<Atom> {
        let (a, b) = match self.selection.take() {
            Some(r) => r,
            None => (self.operand_start(self.cursor.pos, chain), self.cursor.pos),
        };
        self.cursor.pos = a;
        self.cur_mut().atoms.drain(a..b).collect()
    }

    fn wrap_if_compound(atoms: Vec<Atom>) -> Vec<Atom> {
        if atoms.iter().any(|a| matches!(a, Atom::Op(_))) {
            vec![Atom::Paren(Block::new(atoms))]
        } else {
            atoms
        }
    }

    /// Voeg een atoom met plaatsen in en zet de cursor in plaats `slot`.
    fn insert_and_enter(&mut self, a: Atom, slot: usize) {
        self.insert(a);
        let i = self.cursor.pos - 1;
        self.enter(i, slot, false);
    }

    fn has_root_op(&self, op: Op) -> bool {
        self.root.atoms.iter().any(|a| *a == Atom::Op(op))
    }

    /// Verwerk één commando.
    pub fn apply(&mut self, cmd: Cmd) -> Effect {
        match cmd {
            Cmd::Char(c) => self.char(c),
            Cmd::Left => {
                self.left();
                Effect::None
            }
            Cmd::Right => {
                self.right();
                Effect::None
            }
            Cmd::Up => {
                self.vertical(-1);
                Effect::None
            }
            Cmd::Down => {
                self.vertical(1);
                Effect::None
            }
            Cmd::Home => {
                self.selection = None;
                self.cursor = Cursor { path: vec![], pos: 0 };
                Effect::None
            }
            Cmd::End => {
                self.selection = None;
                self.cursor = Cursor { path: vec![], pos: self.root.len() };
                Effect::None
            }
            Cmd::Backspace => {
                self.backspace();
                Effect::None
            }
            Cmd::Delete => {
                self.delete();
                Effect::None
            }
            // de spatiebalk vergroot de selectie, behalve in een tekst: daar is het een spatie
            Cmd::Grow if self.in_string() => {
                self.insert(Atom::Char(' '));
                Effect::None
            }
            Cmd::Grow => {
                self.grow();
                Effect::None
            }
            Cmd::Enter => {
                if self.in_program_line().is_some() {
                    self.add_program_line();
                    Effect::None
                } else {
                    Effect::Commit
                }
            }
            Cmd::AddRow => self.add_row(),
            Cmd::AddColumn => self.add_column(),
            Cmd::Escape => Effect::Commit,
            Cmd::Greek => {
                self.greek();
                Effect::None
            }
            Cmd::Template(t) => self.template(t),
        }
    }

    /// Staat de cursor in een tekst (`"…"`)?
    fn in_string(&self) -> bool {
        matches!(self.parent_atom(), Some(Atom::Str(_)))
    }

    fn char(&mut self, c: char) -> Effect {
        // in een tekst komt elk teken letterlijk; `"` sluit de tekst af
        if self.in_string() {
            if c == '"' {
                let (i, _) = self.cursor.path.pop().expect("ouder");
                self.cursor.pos = i + 1;
            } else {
                self.insert(Atom::Char(c));
            }
            return Effect::None;
        }
        let prev = self.cursor.pos.checked_sub(1).and_then(|k| self.cur().atoms.get(k).cloned());
        match c {
            ' ' => {
                self.grow();
                Effect::None
            }
            ':' => {
                if self.has_root_op(Op::Define) || self.has_root_op(Op::Eval) {
                    return Effect::Unsupported("deze regio heeft al := of =");
                }
                // alles links van de cursor wordt de naam; de cursor gaat naar de rechterkant
                self.selection = None;
                if !self.cursor.path.is_empty() {
                    self.cursor = Cursor { path: vec![], pos: self.root.len() };
                }
                let pos = self.cursor.pos;
                self.root.atoms.insert(pos, Atom::Op(Op::Define));
                self.cursor.pos = pos + 1;
                Effect::None
            }
            '=' => {
                match prev {
                    Some(Atom::Op(Op::Lt)) => return self.replace_prev(Atom::Op(Op::Le)),
                    Some(Atom::Op(Op::Gt)) => return self.replace_prev(Atom::Op(Op::Ge)),
                    _ => {}
                }

                if self.has_root_op(Op::Eval) {
                    return Effect::Commit;
                }
                // achter een pijl: `substitute, x = 2` (een vergelijking, geen uitrekenen)
                if self.has_root_op(Op::Arrow) {
                    return self.op(Op::BoolEq);
                }
                // uitrekenen: `=` achteraan, de cursor in de eenheidsplaats erachter
                self.selection = None;
                self.root.atoms.push(Atom::Op(Op::Eval));
                self.cursor = Cursor { path: vec![], pos: self.root.len() };
                Effect::Commit
            }
            // symbolisch rekenen: `→` achteraan, de cursor erachter voor een sleutelwoord
            '→' => {
                if self.has_root_op(Op::Define) || self.has_root_op(Op::Eval) || self.has_root_op(Op::Arrow) {
                    return Effect::Unsupported("deze regio heeft al := , = of →");
                }
                self.selection = None;
                self.root.atoms.push(Atom::Op(Op::Arrow));
                self.cursor = Cursor { path: vec![], pos: self.root.len() };
                Effect::Commit
            }
            '>' if prev == Some(Atom::Op(Op::Lt)) => self.replace_prev(Atom::Op(Op::Ne)),
            '+' => self.op(Op::Plus),
            '-' | '−' => self.op(Op::Minus),
            '*' => self.op(Op::Times),
            ',' => self.op(Op::Comma),
            '<' => self.op(Op::Lt),
            '>' => self.op(Op::Gt),
            '!' => {
                let taken = Self::wrap_if_compound(self.take_operand(false));
                let n = taken.len();
                let pos = self.cursor.pos;
                self.cur_mut().atoms.splice(pos..pos, taken);
                self.cursor.pos += n;
                self.insert(Atom::Op(Op::Factorial));
                Effect::None
            }
            '/' => {
                let taken = self.take_operand(true);
                let empty = taken.is_empty();
                self.insert_and_enter(Atom::Frac { num: Block::new(taken), den: Block::default() }, if empty { 0 } else { 1 });
                Effect::None
            }
            '^' => self.postfix(|b| Atom::Pow(b)),
            '[' => self.postfix(|b| Atom::Index(b)),
            '\\' => {
                let inner = self.take_selection();
                self.insert_and_enter(Atom::Sqrt(Block::new(inner)), 0);
                self.cursor.pos = self.cur().len();
                Effect::None
            }
            '|' => {
                let inner = self.take_selection();
                self.insert_and_enter(Atom::Abs(Block::new(inner)), 0);
                self.cursor.pos = self.cur().len();
                Effect::None
            }
            '(' => {
                let inner = self.take_selection();
                self.insert_and_enter(Atom::Paren(Block::new(inner)), 0);
                self.cursor.pos = self.cur().len();
                Effect::None
            }
            ')' => {
                // uit de dichtstbijzijnde haakjes stappen
                let mut path = self.cursor.path.clone();
                while let Some((i, _)) = path.pop() {
                    if matches!(self.block(&path).atoms[i], Atom::Paren(_)) {
                        self.cursor = Cursor { path, pos: i + 1 };
                        self.selection = None;
                        return Effect::None;
                    }
                }
                Effect::None
            }
            '"' => {
                if self.root.is_empty() {
                    // in een lege regio: een tekstregio, zoals in Mathcad
                    Effect::ToText
                } else {
                    // in een formule: een tekst als waarde (`s := "abc"`)
                    self.insert(Atom::Str(Block::default()));
                    let i = self.cursor.pos - 1;
                    self.enter(i, 0, false);
                    Effect::None
                }
            }
            '{' => self.template(Template::LocalAssign),
            '}' => self.template(Template::If),
            ']' => self.template(Template::Program),
            '%' | '#' | '$' | '&' | '@' | '~' | '`' | ';' | '?' => Effect::Unsupported("dit teken heeft hier (nog) geen betekenis"),
            c => {
                if self.selection.is_some() {
                    self.delete_selection();
                }
                self.insert(Atom::Char(c));
                Effect::None
            }
        }
    }

    fn take_selection(&mut self) -> Vec<Atom> {
        match self.selection.take() {
            Some((a, b)) => {
                self.cursor.pos = a;
                self.cur_mut().atoms.drain(a..b).collect()
            }
            None => vec![],
        }
    }

    fn replace_prev(&mut self, a: Atom) -> Effect {
        let k = self.cursor.pos - 1;
        self.cur_mut().atoms[k] = a;
        Effect::None
    }

    fn op(&mut self, op: Op) -> Effect {
        // met een selectie: de operator komt achter de selectie
        if let Some((_, b)) = self.selection.take() {
            self.cursor.pos = b;
        }
        self.insert(Atom::Op(op));
        Effect::None
    }

    /// Macht of index: het voorgaande blijft staan, de macht/index komt erachter en krijgt de cursor.
    fn postfix(&mut self, make: impl FnOnce(Block) -> Atom) -> Effect {
        if let Some((a, b)) = self.selection.take() {
            let taken: Vec<Atom> = self.cur_mut().atoms.drain(a..b).collect();
            let taken = Self::wrap_if_compound(taken);
            let n = taken.len();
            self.cur_mut().atoms.splice(a..a, taken);
            self.cursor.pos = a + n;
        }
        self.insert_and_enter(make(Block::default()), 0);
        Effect::None
    }

    fn left(&mut self) {
        if let Some((a, _)) = self.selection.take() {
            self.cursor.pos = a;
            return;
        }
        let pos = self.cursor.pos;
        if pos > 0 {
            let i = pos - 1;
            let n = self.cur().atoms[i].slots().len();
            if n > 0 {
                self.enter(i, n - 1, true);
            } else {
                self.cursor.pos -= 1;
            }
            return;
        }
        // aan het begin van een plaats: naar de vorige plaats van hetzelfde atoom, of eruit
        if let Some(&(i, s)) = self.cursor.path.last() {
            if s > 0 {
                self.cursor.path.pop();
                self.enter(i, s - 1, true);
            } else {
                self.exit(false);
            }
        }
    }

    fn right(&mut self) {
        if let Some((_, b)) = self.selection.take() {
            self.cursor.pos = b;
            return;
        }
        let pos = self.cursor.pos;
        if pos < self.cur().len() {
            if self.cur().atoms[pos].is_structure() {
                self.enter(pos, 0, false);
            } else {
                self.cursor.pos += 1;
            }
            return;
        }
        if let Some(&(i, s)) = self.cursor.path.last() {
            let n = self.parent_atom().map_or(0, |a| a.slots().len());
            if s + 1 < n {
                self.cursor.path.pop();
                self.enter(i, s + 1, false);
            } else {
                self.exit(true);
            }
        }
    }

    /// Omhoog of omlaag: teller ↔ noemer, rijen van een matrix, regels van een programma.
    fn vertical(&mut self, dir: i32) {
        self.selection = None;
        let mut path = self.cursor.path.clone();
        while let Some((i, s)) = path.pop() {
            let atom = &self.block(&path).atoms[i];
            let target = match atom {
                Atom::Frac { .. } => match (s, dir) {
                    (0, 1) => Some(1),
                    (1, -1) => Some(0),
                    _ => None,
                },
                Atom::Matrix { rows, cols, .. } => {
                    let r = s / cols;
                    let nr = r as i32 + dir;
                    (nr >= 0 && (nr as usize) < *rows).then(|| nr as usize * cols + s % cols)
                }
                Atom::Program(lines) => {
                    let n = s as i32 + dir;
                    (n >= 0 && (n as usize) < lines.len()).then_some(n as usize)
                }
                Atom::Ctrl { kind, body, .. } if s >= kind.head_len() => {
                    let n = s as i32 + dir;
                    let lo = kind.head_len() as i32;
                    (n >= lo && n < lo + body.len() as i32).then_some(n as usize)
                }
                _ => None,
            };
            if let Some(t) = target {
                path.push((i, t));
                let len = self.block(&path).len();
                self.cursor = Cursor { path, pos: self.cursor.pos.min(len) };
                return;
            }
        }
    }

    fn all_slots_empty(a: &Atom) -> bool {
        a.slots().iter().all(|b| b.is_empty())
    }

    fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        let pos = self.cursor.pos;
        if pos > 0 {
            let i = pos - 1;
            let atom = self.cur().atoms[i].clone();
            if !atom.is_structure() || Self::all_slots_empty(&atom) {
                self.cur_mut().atoms.remove(i);
                self.cursor.pos -= 1;
            } else {
                let n = atom.slots().len();
                self.enter(i, n - 1, true);
            }
            return;
        }
        if let Some(&(i, s)) = self.cursor.path.last() {
            let parent = self.parent_atom().cloned().expect("ouder");
            if Self::all_slots_empty(&parent) {
                self.cursor.path.pop();
                self.cur_mut().atoms.remove(i);
                self.cursor.pos = i;
            } else if self.cur().is_empty() && matches!(parent, Atom::Frac { .. } | Atom::Root { .. }) {
                // Backspace in een lege noemer (of wortelexponent) haalt de breuk weg; de rest blijft
                let kept: usize = parent.slots().iter().map(|b| b.len()).sum();
                self.unwrap_parent();
                self.cursor.pos = i + kept;
            } else if matches!(parent, Atom::Frac { .. } | Atom::Paren(_) | Atom::Sqrt(_) | Atom::Abs(_)) && s == 0 {
                // Backspace aan het begin van haakjes of een wortel haalt de structuur weg, de inhoud blijft
                self.unwrap_parent();
            } else if s > 0 {
                self.cursor.path.pop();
                self.enter(i, s - 1, true);
            } else {
                self.exit(false);
            }
        }
    }

    /// Vervang het ouderatoom door de inhoud van zijn plaatsen (achter elkaar).
    fn unwrap_parent(&mut self) {
        let (i, _) = self.cursor.path.pop().expect("ouder");
        let atom = self.cur_mut().atoms.remove(i);
        let inner: Vec<Atom> = atom.slots().into_iter().flat_map(|b| b.atoms.clone()).collect();
        self.cur_mut().atoms.splice(i..i, inner);
        self.cursor.pos = i;
    }

    fn delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        let pos = self.cursor.pos;
        if pos < self.cur().len() {
            let atom = self.cur().atoms[pos].clone();
            if !atom.is_structure() || Self::all_slots_empty(&atom) {
                self.cur_mut().atoms.remove(pos);
            } else {
                self.enter(pos, 0, false);
            }
            return;
        }
        if let Some(&(i, s)) = self.cursor.path.last() {
            let parent = self.parent_atom().cloned().expect("ouder");
            if Self::all_slots_empty(&parent) {
                self.cursor.path.pop();
                self.cur_mut().atoms.remove(i);
                self.cursor.pos = i;
            } else if s + 1 < parent.slots().len() {
                self.cursor.path.pop();
                self.enter(i, s + 1, false);
            } else {
                self.exit(true);
            }
        }
    }

    /// Het deel van het huidige blok dat 'de uitdrukking' is: in de wortel zonder `naam :=` en
    /// zonder de `= eenheid` erachter.
    fn expr_range(&self) -> (usize, usize) {
        let b = self.cur();
        if !self.cursor.path.is_empty() {
            return (0, b.len());
        }
        let find = |op| b.atoms.iter().position(|a| *a == Atom::Op(op));
        match (find(Op::Define), find(Op::Eval)) {
            (Some(d), _) if self.cursor.pos > d => (d + 1, b.len()),
            (_, Some(e)) if self.cursor.pos <= e => (0, e),
            (_, Some(e)) => (e + 1, b.len()),
            (Some(d), _) => (0, d),
            _ => (0, b.len()),
        }
    }

    /// Spatie: eerst de operand voor de cursor, dan het product, dan de hele uitdrukking, dan het
    /// omringende atoom, enz.
    fn grow(&mut self) {
        let (lo, hi) = self.expr_range();
        let end = self.cursor.pos;
        let whole = (0, self.root.len());
        match self.selection {
            // de hele formule was geselecteerd: opnieuw beginnen, zoals in Mathcad
            Some(s) if self.cursor.path.is_empty() && s == whole => self.selection = None,
            None if end > lo => {
                let a = self.operand_start(end, false).max(lo);
                self.selection = Some((a, end));
            }
            Some((a, b)) if self.operand_start(b, true).max(lo) < a => {
                self.selection = Some((self.operand_start(b, true).max(lo), b));
            }
            Some((a, b)) if (a, b) != (lo, hi) && hi > lo => {
                self.selection = Some((lo, hi));
                self.cursor.pos = hi;
            }
            _ => {
                if let Some((i, _)) = self.cursor.path.pop() {
                    self.cursor.pos = i + 1;
                    self.selection = Some((i, i + 1));
                } else if !self.root.is_empty() {
                    // de laatste stap: de hele formule, met `naam :=` of `= eenheid` erbij
                    self.selection = Some(whole);
                    self.cursor.pos = whole.1;
                }
            }
        }
    }

    /// Zet de letter voor de cursor om in de Griekse letter (en terug), zoals Ctrl+G in Mathcad.
    fn greek(&mut self) {
        let pos = self.cursor.pos;
        if pos == 0 {
            return;
        }
        if let Atom::Char(c) = self.cur().atoms[pos - 1] {
            if let Some(g) = toggle_greek(c) {
                self.cur_mut().atoms[pos - 1] = Atom::Char(g);
            }
        }
    }

    /// Staat de cursor (direct) in een regel van een programma? Geeft dan (pad naar het programma-atoom, regelnummer).
    fn in_program_line(&self) -> Option<(Vec<(usize, usize)>, usize, usize)> {
        let (last, rest) = self.cursor.path.split_last()?;
        match &self.block(rest).atoms[last.0] {
            Atom::Program(_) => Some((rest.to_vec(), last.0, last.1)),
            Atom::Ctrl { kind, .. } if last.1 >= kind.head_len() => Some((rest.to_vec(), last.0, last.1)),
            _ => None,
        }
    }

    fn add_program_line(&mut self) {
        if let Some((parent, i, s)) = self.in_program_line() {
            let atom = &mut self.block_mut(&parent).atoms[i];
            match atom {
                Atom::Program(lines) => lines.insert(s + 1, Block::default()),
                Atom::Ctrl { body, kind, .. } => body.insert(s + 1 - kind.head_len(), Block::default()),
                _ => return,
            }
            let mut path = parent;
            path.push((i, s + 1));
            self.cursor = Cursor { path, pos: 0 };
            self.selection = None;
        }
    }

    fn add_row(&mut self) -> Effect {
        let mut path = self.cursor.path.clone();
        while let Some((i, s)) = path.pop() {
            if let Atom::Matrix { rows, cols, cells } = &mut self.block_mut(&path).atoms[i] {
                let r = s / *cols;
                let at = (r + 1) * *cols;
                for _ in 0..*cols {
                    cells.insert(at, Block::default());
                }
                *rows += 1;
                path.push((i, at));
                self.cursor = Cursor { path, pos: 0 };
                return Effect::None;
            }
        }
        if self.in_program_line().is_some() {
            self.add_program_line();
            return Effect::None;
        }
        Effect::Unsupported("Shift+Enter voegt een rij toe in een matrix of een regel in een programma")
    }

    fn add_column(&mut self) -> Effect {
        let mut path = self.cursor.path.clone();
        while let Some((i, s)) = path.pop() {
            if let Atom::Matrix { rows, cols, cells } = &mut self.block_mut(&path).atoms[i] {
                let c = s % *cols;
                for r in (0..*rows).rev() {
                    cells.insert(r * *cols + c + 1, Block::default());
                }
                *cols += 1;
                let r = s / (*cols - 1);
                path.push((i, r * *cols + c + 1));
                self.cursor = Cursor { path, pos: 0 };
                return Effect::None;
            }
        }
        Effect::Unsupported("Shift+Spatie voegt een kolom toe in een matrix")
    }

    /// Zorg dat de cursor in een programmaregel staat: zo niet, maak dan een programma op de cursor.
    fn ensure_program_line(&mut self) {
        if self.in_program_line().is_some() {
            return;
        }
        self.insert_and_enter(Atom::Program(vec![Block::default()]), 0);
    }

    fn template(&mut self, t: Template) -> Effect {
        let e = |n: usize| (0..n).map(|_| Block::default()).collect::<Vec<_>>();
        match t {
            Template::Sum | Template::Prod => {
                let kind = if t == Template::Sum { Big::Sum } else { Big::Prod };
                self.insert_and_enter(Atom::Big { kind, slots: e(4) }, 0);
            }
            Template::Integral => self.insert_and_enter(Atom::Big { kind: Big::Integral, slots: e(4) }, 0),
            Template::Deriv => {
                // d/d□ □: de cursor op de afgeleide naar ...; de orde-plaats blijft leeg (1e afgeleide)
                self.insert_and_enter(Atom::Big { kind: Big::Deriv, slots: e(3) }, 0);
            }
            Template::Matrix => self.insert_and_enter(Atom::Matrix { rows: 2, cols: 2, cells: e(4) }, 0),
            Template::Program => {
                let inner = self.take_selection();
                let mut lines = vec![Block::new(inner), Block::default()];
                if lines[0].is_empty() {
                    lines.truncate(2);
                }
                self.insert_and_enter(Atom::Program(lines), 0);
                self.cursor.pos = self.cur().len();
            }
            Template::If | Template::ElseIf | Template::Else | Template::While | Template::For => {
                self.ensure_program_line();
                let kind = match t {
                    Template::If => Ctrl::If,
                    Template::ElseIf => Ctrl::ElseIf,
                    Template::Else => Ctrl::Else,
                    Template::While => Ctrl::While,
                    _ => Ctrl::For,
                };
                self.insert_and_enter(Atom::Ctrl { kind, head: e(kind.head_len()), body: e(1) }, 0);
            }
            Template::Break | Template::Continue => {
                self.ensure_program_line();
                self.insert(Atom::Kw(if t == Template::Break { Kw::Break } else { Kw::Continue }));
            }
            Template::Return => {
                self.ensure_program_line();
                self.insert(Atom::Kw(Kw::Return));
            }
            Template::LocalAssign => {
                self.ensure_program_line();
                self.insert(Atom::Op(Op::Assign));
            }
            Template::Op(op) => return self.op(op),
            Template::Infinity => self.insert(Atom::Char('∞')),
            Template::NthRoot => {
                let inner = self.take_selection();
                self.insert_and_enter(Atom::Root { index: Block::default(), radicand: Block::new(inner) }, 0);
            }
            Template::Abs => return self.char('|'),
        }
        Effect::None
    }
}

/// Latijnse letter ↔ Griekse letter, zoals Ctrl+G in Mathcad.
pub fn toggle_greek(c: char) -> Option<char> {
    const PAIRS: &[(char, char)] = &[
        ('a', 'α'), ('b', 'β'), ('c', 'χ'), ('d', 'δ'), ('e', 'ε'), ('f', 'φ'), ('g', 'γ'), ('h', 'η'),
        ('i', 'ι'), ('j', 'ϕ'), ('k', 'κ'), ('l', 'λ'), ('m', 'μ'), ('n', 'ν'), ('o', 'ο'), ('p', 'π'),
        ('q', 'θ'), ('r', 'ρ'), ('s', 'σ'), ('t', 'τ'), ('u', 'υ'), ('v', 'ϖ'), ('w', 'ω'), ('x', 'ξ'),
        ('y', 'ψ'), ('z', 'ζ'), ('A', 'Α'), ('B', 'Β'), ('C', 'Χ'), ('D', 'Δ'), ('E', 'Ε'), ('F', 'Φ'),
        ('G', 'Γ'), ('H', 'Η'), ('I', 'Ι'), ('J', 'ϑ'), ('K', 'Κ'), ('L', 'Λ'), ('M', 'Μ'), ('N', 'Ν'),
        ('O', 'Ο'), ('P', 'Π'), ('Q', 'Θ'), ('R', 'Ρ'), ('S', 'Σ'), ('T', 'Τ'), ('U', 'Υ'), ('W', 'Ω'),
        ('X', 'Ξ'), ('Y', 'Ψ'), ('Z', 'Ζ'),
    ];
    PAIRS
        .iter()
        .find_map(|&(l, g)| if c == l { Some(g) } else if c == g { Some(l) } else { None })
}
