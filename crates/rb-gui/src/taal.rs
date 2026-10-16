//! De taal van de interface: Nederlands (standaard) of Engels. Alleen de bediening (menu's, lint,
//! instellingen, dialogen, statusbalk); meldingen van de rekenkern blijven Nederlands.
//!
//! De Nederlandse tekst is de sleutel: `t("Opslaan")` geeft "Save" als de taal Engels is. Een tekst die
//! niet in de tabel staat, blijft zoals hij is.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static ENGELS: AtomicBool = AtomicBool::new(false);

pub fn set_english(on: bool) {
    ENGELS.store(on, Ordering::Relaxed);
}

pub fn english() -> bool {
    ENGELS.load(Ordering::Relaxed)
}

/// De tekst in de gekozen taal.
pub fn t(nl: &str) -> &str {
    if !english() {
        return nl;
    }
    static TABEL: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    TABEL.get_or_init(|| VERTALING.iter().copied().collect()).get(nl).copied().unwrap_or(nl)
}

/// Kies tussen een Nederlandse en een Engelse tekst (voor zinnen met getallen erin).
pub fn nl_en<'a>(nl: &'a str, en: &'a str) -> &'a str {
    if english() {
        en
    } else {
        nl
    }
}

const VERTALING: &[(&str, &str)] = &[
    // menu's
    ("Bestand", "File"),
    ("Beeld", "View"),
    ("Invoegen", "Insert"),
    ("Help", "Help"),
    ("Nieuw uit sjabloon", "New from template"),
    ("Opslaan als Mathcad (.mcdx)…", "Save as Mathcad (.mcdx)…"),
    ("Donkere modus", "Dark mode"),
    ("Afbeelding…", "Image…"),
    ("Verslag", "Report"),
    ("Tentamen", "Exam"),
    ("Ligger op twee steunpunten", "Simply supported beam"),
    ("Warmtebalans", "Heat balance"),
    // acties (keymap.rs)
    ("Nieuw werkblad", "New worksheet"),
    ("Openen…", "Open…"),
    ("Opslaan", "Save"),
    ("Opslaan als…", "Save as…"),
    ("Exporteren naar PDF…", "Export to PDF…"),
    ("Exporteren naar SVG…", "Export to SVG…"),
    ("Bron tonen of verbergen", "Show or hide source"),
    ("Groter", "Zoom in"),
    ("Kleiner", "Zoom out"),
    ("Alles herberekenen", "Recalculate all"),
    ("Letter voor de cursor ↔ Griekse letter", "Letter before cursor ↔ Greek letter"),
    ("Sneltoetsen…", "Keyboard shortcuts…"),
    ("Syntax en hulp", "Syntax and help"),
    ("Welkomstblad (voorbeeld)", "Welcome sheet (example)"),
    ("Instellingen…", "Settings…"),
    ("Zoeken…", "Find…"),
    ("Vervangen of hernoemen…", "Replace or rename…"),
    ("Afdrukvoorbeeld", "Print preview"),
    ("Afdrukken…", "Print…"),
    // dialogen
    ("Niet-opgeslagen wijzigingen", "Unsaved changes"),
    ("Niet opslaan", "Don't save"),
    ("Annuleren", "Cancel"),
    ("Sneltoetsen", "Keyboard shortcuts"),
    ("Bestand met de standaardtoetsen aanmaken", "Create file with the default shortcuts"),
    ("Opnieuw inlezen", "Reload"),
    ("Mathcad: wat niet helemaal overkwam", "Mathcad: what did not fully carry over"),
    ("Herstellen", "Restore"),
    ("Weggooien", "Discard"),
    // afdrukvoorbeeld en statusbalk
    ("PDF maken…", "Create PDF…"),
    ("Terug naar bewerken (Esc)", "Back to editing (Esc)"),
    ("Naar de eerste fout", "Go to the first error"),
    ("Groter (Ctrl+wiel)", "Zoom in (Ctrl+wheel)"),
    ("Kleiner (Ctrl+wiel)", "Zoom out (Ctrl+wheel)"),
    ("Terug naar 100%", "Back to 100%"),
    ("Naar de downloadpagina", "Go to the download page"),
    // zoeken
    ("Zoeken", "Find"),
    ("Vorige (Shift+Enter)", "Previous (Shift+Enter)"),
    ("Volgende (Enter)", "Next (Enter)"),
    ("Hele namen", "Whole names"),
    ("Alleen variabelen en functies met precies deze naam, niet stukjes tekst", "Only variables and functions with exactly this name, not parts of text"),
    ("Vervangen", "Replace"),
    ("Nieuwe naam", "New name"),
    ("Vervangen door", "Replace with"),
    ("Hernoem overal", "Rename everywhere"),
    ("Vervang alles", "Replace all"),
    ("Sluiten (Esc)", "Close (Esc)"),
    ("geen treffers", "no matches"),
    // lint: tabbladen
    ("Wiskunde", "Math"),
    ("Operatoren", "Operators"),
    ("Symbolen", "Symbols"),
    ("Functies", "Functions"),
    ("Matrices", "Matrices"),
    ("Programmeren", "Programming"),
    ("Eenheden", "Units"),
    ("Berekenen", "Calculate"),
    ("Document", "Document"),
    // lint: groepen
    ("Formules", "Formulas"),
    ("Regio's", "Regions"),
    ("Definities en uitkomsten", "Definitions and results"),
    ("Rekenen", "Arithmetic"),
    ("Bewerken", "Edit"),
    ("Rekenkundig", "Arithmetic"),
    ("Analyse", "Calculus"),
    ("Vergelijken", "Comparison"),
    ("Logisch", "Logic"),
    ("Symbolisch", "Symbolic"),
    ("Griekse letters (of letter + Ctrl+G)", "Greek letters (or letter + Ctrl+G)"),
    ("Constanten", "Constants"),
    ("Alle functies", "All functions"),
    ("Voorwaarden", "Conditions"),
    ("Lussen", "Loops"),
    ("Herberekenen", "Recalculate"),
    ("Pagina's", "Pages"),
    ("Thema", "Theme"),
    ("Achtergrond van de pagina", "Page background"),
    ("Uitvoer", "Output"),
    ("Bron", "Source"),
    ("Goniometrie", "Trigonometry"),
    ("Machten en logaritmen", "Powers and logarithms"),
    ("Afronden", "Rounding"),
    ("Statistiek", "Statistics"),
    ("Oplossen", "Solving"),
    ("Interpolatie", "Interpolation"),
    ("Bestanden (Excel, CSV)", "Files (Excel, CSV)"),
    // lint: opschriften onder de knoppen
    ("Bibliotheek", "Library"),
    ("Omwerken ▾", "Rearrange ▾"),
    ("Tekst", "Text"),
    ("Grafiek", "Plot"),
    ("Tabel", "Table"),
    ("Afbeelding", "Image"),
    ("Definitie", "Definition"),
    ("Uitrekenen", "Evaluate"),
    ("Lokaal", "Local"),
    ("Bereik", "Range"),
    ("Breuk", "Fraction"),
    ("Macht", "Power"),
    ("Wortel", "Root"),
    ("n-de wortel", "nth root"),
    ("Absoluut", "Absolute"),
    ("Som", "Sum"),
    ("Integraal", "Integral"),
    ("Afgeleide", "Derivative"),
    ("Ongedaan", "Undo"),
    ("Opnieuw", "Redo"),
    ("Knippen", "Cut"),
    ("Kopiëren", "Copy"),
    ("Plakken", "Paste"),
    ("Gebied", "Area"),
    ("Pagina-einde", "Page break"),
    ("Koptekst", "Header"),
    ("Voettekst", "Footer"),
    // lint: tooltips
    ("Nieuwe wiskunderegio (Ctrl+Shift+M); of klik ergens en typ", "New math region (Ctrl+Shift+M); or click anywhere and type"),
    ("Tekstregio (Ctrl+T of \")", "Text region (Ctrl+T or \")"),
    ("XY-grafiek (Ctrl+2)", "XY plot (Ctrl+2)"),
    ("Tabel met kolommen en eenheden (Ctrl+6)", "Table with columns and units (Ctrl+6)"),
    ("PNG-afbeelding invoegen (of sleep een bestand naar het venster)", "Insert a PNG image (or drag a file onto the window)"),
    ("Uitrekenen (=); typ daarna een eenheid", "Evaluate (=); then type a unit"),
    ("Lokale toekenning in een programma ({)", "Local assignment in a program ({)"),
    ("Breuk (/)", "Fraction (/)"),
    ("Macht (^)", "Power (^)"),
    ("Absolute waarde (|)", "Absolute value (|)"),
    ("Som (Ctrl+Shift+4)", "Sum (Ctrl+Shift+4)"),
    ("Integraal (Ctrl+Shift+I)", "Integral (Ctrl+Shift+I)"),
    ("Afgeleide (Ctrl+Shift+D)", "Derivative (Ctrl+Shift+D)"),
    ("Ongedaan maken (Ctrl+Z)", "Undo (Ctrl+Z)"),
    ("Opnieuw (Ctrl+Y)", "Redo (Ctrl+Y)"),
    ("Knippen (Ctrl+X)", "Cut (Ctrl+X)"),
    ("Kopiëren (Ctrl+C)", "Copy (Ctrl+C)"),
    ("Plakken (Ctrl+V)", "Paste (Ctrl+V)"),
    ("Symbolisch uitrekenen (Ctrl+.)", "Evaluate symbolically (Ctrl+.)"),
    ("Vereenvoudigen", "Simplify"),
    ("Uitwerken (haakjes wegwerken)", "Expand (remove parentheses)"),
    ("Ontbinden in factoren", "Factor"),
    ("Oplossen naar een variabele; typ daarna de naam", "Solve for a variable; then type its name"),
    ("Invullen: typ daarna x = waarde", "Substitute: then type x = value"),
    ("Limiet: typ daarna x = punt (ook ∞)", "Limit: then type x = point (also ∞)"),
    ("Taylorreeks: typ daarna x = punt, orde", "Taylor series: then type x = point, order"),
    ("Als decimaal getal: typ daarna het aantal cijfers", "As a decimal number: then type the number of digits"),
    ("Exporteren naar PDF (Ctrl+Shift+P)", "Export to PDF (Ctrl+Shift+P)"),
    ("Exporteren naar SVG", "Export to SVG"),
    ("Opslaan als Mathcad Prime-bestand (.mcdx)", "Save as a Mathcad Prime file (.mcdx)"),
    // instellingen
    ("Instellingen", "Settings"),
    ("Uiterlijk", "Appearance"),
    ("Achtergrond", "Background"),
    ("Zoom bij het starten", "Zoom at startup"),
    ("Starten", "Startup"),
    ("Bij het starten", "At startup"),
    ("Leeg blad", "Blank sheet"),
    ("Welkomstblad", "Welcome sheet"),
    ("Rekenen en typen", "Calculation and typing"),
    ("Cijfers in uitkomsten", "Digits in results"),
    ("Automatisch aanvullen", "Autocomplete"),
    ("lijst met namen tijdens het typen", "list of names while typing"),
    ("Nieuwe versies", "New versions"),
    ("eens per dag kijken op GitHub", "check GitHub once a day"),
    ("Vraagt alleen het nummer van de nieuwste versie op; er gaat niets van jou mee", "Only asks for the number of the newest version; nothing of yours is sent"),
    ("Taal", "Language"),
    ("Interface", "Interface"),
    ("Wijzigingen werken meteen. Sneltoetsen staan in keymap.txt (Help → Sneltoetsen).", "Changes apply immediately. Shortcuts are in keymap.txt (Help → Keyboard shortcuts)."),
    ("☀ Licht", "☀ Light"),
    ("🌙 Donker", "🌙 Dark"),
    ("Zoals Windows", "Like Windows"),
    ("Leeg", "Blank"),
    ("Raster", "Dots"),
    ("Ruitjes", "Squares"),
    ("Lijnen", "Lines"),
    // leeg blad
    ("Klik ergens en begin te typen", "Click anywhere and start typing"),
    ("Of open een bestand met Ctrl+O. Een voorbeeld staat onder Help → Welkomstblad.", "Or open a file with Ctrl+O. There is an example under Help → Welcome sheet."),
    ("Recent geopend", "Recently opened"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertalen() {
        set_english(false);
        assert_eq!(t("Opslaan"), "Opslaan");
        set_english(true);
        assert_eq!(t("Opslaan"), "Save");
        assert_eq!(t("onbekende tekst"), "onbekende tekst");
        assert_eq!(nl_en("fout", "error"), "error");
        set_english(false);
        // geen dubbele sleutels in de tabel
        let mut keys: Vec<&str> = VERTALING.iter().map(|(k, _)| *k).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "dubbele sleutel in VERTALING");
    }
}
