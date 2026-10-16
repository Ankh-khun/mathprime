# Meehelpen aan MathPrime

Fijn dat je wilt helpen. Een paar afspraken:

## Bouwen en testen

```powershell
cargo test --workspace
cargo run -p rb-gui
```

Elke wijziging laat `cargo test --workspace` groen en de release-build zonder warnings
(`cargo build --release` met `RUSTFLAGS=-D warnings`, zoals de CI op GitHub doet).

Heb je Mathcad Prime-bladen (.mcdx), dan vergelijkt deze test MathPrime met de uitkomsten die Mathcad
zelf opsloeg:

```powershell
$env:MATHPRIME_MCDX_DIRS = "C:\pad\naar\bladen"
cargo test -p rb-mcdx -- --nocapture
```

## Regels

- **Geen code of bestanden van PTC.** Alleen het open .mcdx-formaat en de gedocumenteerde Automation
  API (alleen in `tools/`).
- **Geen nieuwe crates van buiten** zonder overleg: MathPrime bouwt bewust op een kleine set
  afhankelijkheden (egui, eframe, egui_plot, miniz_oxide).
- Code en commentaar zijn in het Nederlands, net als de interface; volg de stijl van de code eromheen.
- Een nieuwe functie krijgt een test, en een regel in `README.md` en `docs/INVENTARIS.md`.

## Indeling

Zie het hoofdstuk "Indeling" in `README.md` en `docs/PLAN.md` voor de opbouw van de crates.

## Fouten melden

Open een issue op GitHub met het werkblad (`.rkn`) of een klein voorbeeld dat het probleem laat zien,
wat je verwachtte en wat MathPrime gaf.
