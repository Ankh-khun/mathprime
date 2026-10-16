# MathPrime — architectuur en plan

Doel: een eigen rekenblad-programma in Rust dat de rekenkracht en bediening van Mathcad Prime benadert,
met eenheden als kernfunctie en een open, git-vriendelijk formaat. Dit document beschrijft wat er staat,
hoe het in elkaar zit, wat er nog moet komen en welke keuzes bewust zijn gemaakt.

## 1. Eerlijke omvang

"Alles van Mathcad met dezelfde sneltoetsen" is haalbaar als eindbestemming, maar het is werk van jaren
voor één persoon. De lagen verschillen enorm in moeilijkheid:

| laag | omvang | opmerking |
|---|---|---|
| rekenkern + eenheden | **klaar in v0.1** | de basis, goed getest |
| werkblad + herberekening | **klaar in v0.1** (tekstvorm) | posities van regio's sinds v0.2, nog geen grafische layout |
| taal compleet (fase 1) | **klaar in v0.2** | logica, bereiken, matrices, calculus, complex, temperaturen |
| functiebibliotheek en oplossers (fase 2) | **klaar in v0.3** | programmablokken, lineaire algebra, verdelingen, regressie, solve blocks, ODE's, FFT; rest zie `INVENTARIS.md` |
| grafische interface met formuleweergave (fase 3) | **klaar in v0.4** | grafieken, PDF/SVG |
| werken zoals Mathcad Prime | **klaar in v0.5** | typen op het werkblad met de toetsen van Prime (`rb-edit`), regio's op A4-pagina's met raster, slepen/selecteren/ongedaan maken, lint met tabbladen; lichte en donkere modus |
| `.mcdx` openen | **klaar in v0.6** | crate `rb-mcdx`: formules, tekst, grafieken, solve blocks en afbeeldingen; getoetst tegen de uitkomsten die Mathcad in 11 echte bladen bewaarde (107 van 107 gelijk). Alleen lezen. |
| structuurgebaseerde formule-editor met Prime-gedrag | XL | het moeilijkste deel; veel details |
| symbolisch rekenen | XL | een eigen CAS; alleen beperkt doen of een engine inbinden |
| `.mcdx`-compatibiliteit | L en nooit volledig | reverse engineering; eerst juridisch checken |

(S/M/L/XL zijn relatieve maten, geen weken.)

## 2. Wat er nu staat (v1.5)

**Nieuw in v1.5.** GitHub Actions: `ci.yml` (tests en een release-build met `-D warnings` op windows-latest, bij
elke push naar main en elk pull request) en `release.yml` (bij een tag `v*`: versie tegen `Cargo.toml`, tests,
installatiepakket met IExpress als dat op de runner staat, altijd een zip, `gh release create` met de notities uit
`docs/releases/<tag>.md`). Updatemelding (`rb-gui/src/update.rs`): eens per dag (`update.txt`) op de achtergrond
`curl.exe` naar `releases/latest`; `tag_name` en `html_url` uit de JSON, alleen een hogere versie geeft de melding
rechtsonder. Geen verbinding of een privé-repository (404): niets. Uit te zetten (`updates = uit`).

**Nieuw in v1.4.** Afdrukvoorbeeld (Ctrl+Alt+P, `cv.preview` in `canvas.rs`): de layout met
`Setup { static_plots: true }`, dezelfde als voor de PDF; de layoutcache houdt rekening met de stand; bewerken,
kruis en aanwijzen staan uit, Esc gaat terug. Grafiekopties na de laatste `|` (`plot.rs::parse_options`):
`log x`/`log y`/`log xy` en `x = a ; b`/`y = a ; b`. Bij een logaritmische x-as worden de punten geometrisch
verdeeld; de weergaven tekenen log10 van de waarden met strepen op 1..9 × 10^k (`doc.rs::log_ticks`, ook als
egui_plot-gridspacer) en labels op de machten van 10. Vaste grenzen zijn in de interface de begin- en
resetgrenzen (zoomen kan nog), in de PDF wordt wat erbuiten valt afgeknipt (`clip`, Liang–Barsky). Sneltoetsen met
meer modifiers worden eerst gecontroleerd, want egui laat Ctrl+P ook bij Ctrl+Alt+P afgaan.

**Nieuw in v1.3.** Eén venster (`rb-gui/src/instance.rs`): het eerste venster luistert op 127.0.0.1 (poort en
sleutel in `instance.txt`); een tweede start geeft het bestand door en stopt; bij het naar voren halen geeft het
werkblad zijn toetsenbordfocus op, anders schuift egui het naar beneden. Automatisch bewaren (`recovery.rs`): elke
minuut een kopie per gewijzigd tabblad in `herstel\`, netjes afsluiten leegt de map (behalve als de hersteldvraag
nog open staat), na een crash de vraag herstellen of weggooien. Zoeken en vervangen (`find.rs`, Ctrl+F/Ctrl+H):
hele namen via de tokens van de lexer; hernoemen alleen in de definitie en de regio's die ervan afhangen (graaf).

**Nieuw in v1.2** (uiterlijk). Pictogrammen uit Tabler Icons (outline, MIT; `rb-gui/src/icons.rs`, lettertype als
terugval achter de gewone lettertypen, zodat een pictogram ook in tekst kan staan). Lint zoals Mathcad Prime:
tabbladen met een streep (`underline_tab`), knoppen met pictogram of groot symbool en een opschrift eronder
(`ribbon_button`, `look` in `ribbon.rs`), groepen met bijschrift en scheidingslijn (`ribbon_group`); lange lijsten
(functies, eenheden) blijven compact. Tabbladen als echte tabs (actief in de kleur van het werkblad, oranje stip
voor wijzigingen, × bij aanwijzen, middelste muisknop sluit, pad als tooltip). Statusbalk met pictogram; "n fouten"
springt naar de eerste fout (`goto_first_error`); zoom rechtsonder. Fouten en waarschuwingen met een gekleurde streep
links langs de regio (ook in PDF) in plaats van ✗ (zat niet in het lettertype). Zachte paginaschaduw, één
accentkleur (`icons::ACCENT`) en afgeronde hoeken in de hele interface (`set_dark`). Instellingen in secties. Een
leeg blad toont een aanwijzing en de recent geopende bestanden (`recent.txt`).

**Nieuw in v1.1** (verbeterpunten van de gebruiker). Tabbladen (`rb-gui/src/tabs.rs`): het actieve blad blijft
in de velden van `App`, de andere in `App::tabs` (`DocState`); wisselen ruilt de velden, zodat de rest van de code
niet verandert. Ctrl+Tab/Ctrl+Shift+Tab, Ctrl+W sluit een tabblad (vroeger het venster), afsluiten vraagt per
tabblad met wijzigingen. Instellingen (`rb-gui/src/settings.rs`, Ctrl+,): thema met "zoals Windows"
(`ctx.system_theme()`), achtergrond, start leeg of welkomstblad, startzoom, significante cijfers
(`rb_units::set_significant_digits`), automatisch aanvullen; oude `instellingen.txt` blijven werken. Start met
een leeg blad. Spatie selecteert als laatste stap de hele formule (`Editor::grow`). Delete buiten een formule
schuift het blad niet meer naar een oud kruisje (`reveal` alleen bij verplaatsen of typen). Ruitjes, lijnen en
puntjes hebben eigen kleuren in donkere modus (omkeren met `darken` maakte ze even donker als de pagina). De
groep Pagina's onder Document was onzichtbaar (vroege `return`); nu zichtbaar.

**Nieuw in v1.0.** Automatisch aanvullen (`rb-edit/src/complete.rs`: de naam voor de cursor, voorstellen uit het
blad, `builtins::NAMES` en de eenheden met voorvoegsels; in `canvas.rs` een lijst onder de cursor met Tab, ↑↓,
Ctrl+Spatie). Tabellen (`rb-sheet/src/table.rs`, `Kind::Table`): elke kolom een gewone definitie
`x := [..] * (eenheid)`; de graaf kent meer namen per regio (`Graph::also_defines`) en de cache meer definities
(`Cached::defined: Vec<Defined>`); weergave als raster (`Content::Grid`), bewerken in een celbewerker
(`Mode::Table`); naar .mcdx als een definitie per kolom. Gebieden, pagina-einden, kop- en voetteksten
(`Kind::Area`, `AreaEnd`, `PageBreak`, `HeaderFooter`): de layout (`layout_full`) verbergt ingeklapte gebieden en
schuift wat eronder staat op; die verschuivingen staan in `Grid::shifts`, zodat `Grid::point`/`cell` op het
scherm en in de bron dezelfde rij geven (anders verspringen regio's bij het opslaan). Pagina-einde naar en van
.mcdx als `<pageBreak />` (Mathcad bewaart het); gebieden en kop/voet nog als tekst, want het Prime-formaat
daarvan staat niet in de meegeleverde schema's.

**Nieuw in v0.9.** Symbolisch rekenen: `rb-core/src/symbolic.rs` (veeltermnormaalvorm met exacte breuken,
simplify/expand/factor/solve/substitute, `deriv` en veeltermintegralen), in de taal als `Stmt::Symbolic` met
`SymOp` (lexer `Tok::Arrow`: `->` of `→`), uitkomst `Outcome::Symbolic`/`Output::Symbolic` die `rb-layout`
opmaakt. Editor: `Op::Arrow` (Ctrl+.), lint-groep "Symbolisch". .mcdx: `ml:symEval` met
`<ml:command><ml:apply>sleutelwoord argument</ml:apply></ml:command>` — die vorm bewaart Mathcad Prime 10
zelf bij opslaan (getest via de Automation API; een kaal argument naast het sleutelwoord gooit Mathcad weg).
Excel/CSV: `rb-sheet/src/io.rs` (READCSV/READEXCEL/READFILE en WRITE…), ZIP/XML in de crate `rb-zipxml`.

**Nieuw in v0.8.** Tekst als waarde (`Value::Str`, `rb-core/src/strings.rs`: concat, strlen, substr, search,
str2num, num2str, str2vec, vec2str, IsString, error). Opslaan als .mcdx (`rb-mcdx/src/write.rs`, `math_out.rs`,
ZIP-schrijver in `zip.rs`): formules, tekst, afbeeldingen, grafieken, solve blocks; labels per positie (een naam is pas
een variabele na zijn definitie). Vergelijken met Mathcad: `rb-mcdx/src/compare.rs` (uitkomsten met eenheid in SI,
foutmeldingen, premium-functies apart), `mathprime-cli vergelijk`, `tools/mathcad-vergelijk.ps1` met het corpus in
`tests/corpus` (Mathcad Prime 10 via de Automation API: Open, Activate, ResumeCalculation, Synchronize, SaveAs).

**Nieuw in v0.7.** Formulebibliotheek (`rb-gui/src/formules.txt`, eigen formules in de instellingenmap) en omwerken
naar een onbekende (`rb-core/src/solve.rs`: symbolisch als de onbekende één keer voorkomt, anders een solve block).
In de eenheid achter `=` gaat een eenheidsnaam voor een variabele met dezelfde naam (zoals in Mathcad). Naast elkaar
schrijven bindt sterker dan * en / (`230 V / 50 Ω`). Ampère heeft een eigen weergavenaam.

**Nieuw in v0.6.** Mathcad Prime-bestanden openen (`rb-mcdx`, geen nieuwe afhankelijkheden). In de taal:
`a := expr = eenheid` (definiëren en tonen), formules die alleen getoond worden (`Kind::Formula`), afbeeldingen
(`image BxH data:image/png;base64,...`, ook in PDF en SVG), grafieken over een bereikvariabele. Naam MathPrime,
eigen pictogram, donkere modus.

**Nieuw in v0.5.** Het werkblad is een canvas zoals in Prime: regio's staan op een raster op A4-pagina's
(`@kolom,rij` in de bron; een regio die over een andere valt schuift omlaag). De formule-editor `rb-edit`: een
boom van atomen (breuk, macht, index, wortel, matrix, Σ/∏/∫/d/dx, programma's) met placeholders en een cursor.
Het toetsgedrag is getest (`/` neemt het product ervoor, Spatie vergroot de selectie, Backspace haalt structuur
weg), en de formule wordt teruggeschreven naar `.rkn`-tekst. De toetsentabel volgt de sneltoetsenlijst van
Prime 10. In `rb-gui`: canvas, lint met tabbladen, achtergrond (leeg/raster/ruitjes/lijnen), testhaak
`MATHPRIME_KEYS` die toetsen en muisklikken afspeelt. Uitkomsten zonder eigen eenheid krijgen een
samengestelde eenheid zoals Mathcad (`kg/s^2` → `N/m`), en grote getallen worden getoond als 3.6·10⁷.

Zeven crates. De rekenkern (`rb-units`, `rb-core`, `rb-sheet`) en de formulelayout (`rb-layout`) hebben
nog steeds geen externe dependencies; de export gebruikt `ttf-parser`, `epaint_default_fonts` en
`miniz_oxide`, de interface `eframe` (egui, OpenGL), `egui_plot` en `rfd` (alle MIT/Apache-achtig).
134 tests slagen. De getallen in `examples/` zijn
onafhankelijk nagerekend: in v0.1 met Python (buigspanning 12.5 MPa, doorbuiging 0.41336 mm, koppel
98.7858 N·m, sweepwaarden), in v0.2 en v0.3 met losse rekenregels in PowerShell (impedantie
10 − 16.1230i Ω, 80 °C = 176 °F, ΔL = 1.44 mm; eigenfrequenties 2.19947 en 5.75828 Hz, knikkracht
942.234 kN, veerstijfheid 1.98 N/mm, gedempte trilling u(1 s) = −3.36852 mm uit de analytische oplossing).
Speciale functies en verdelingen zijn getoetst aan tabelwaarden (Abramowitz & Stegun) en de
verdelingsfuncties bovendien aan de integraal van hun eigen dichtheid. De PDF-export is gecontroleerd
door hem met de eigen PDF-renderer van Windows (`Windows.Data.Pdf`) naar een afbeelding om te zetten; de
interface met schermafdrukken via `MATHPRIME_SCREENSHOT` (zie fase 3).

```
rb-units  →  rb-core  →  rb-sheet  →  rb-layout  →  rb-export  →  rb-cli (mathprime-cli)
dimensies    lexer        regio's       vakken        weergave       run / deps / repl / export
eenheden     parser       posities      breuken       pagina's
grootheden   waarden      graaf         machten       grafieken   →  rb-gui (mathprime)
weergave     evaluator    herberekening wortels       SVG            bron + werkblad
temperatuur  functies     sweeps        matrices      PDF            interactieve grafieken
             numeriek     solve blocks  Σ ∏ ∫         lettertypen    sneltoetsen uit keymap.txt
             linalg       grafieken     programma's
             special
             fit
             eval/solve
```

**Formulelayout** (`rb-layout`). Zet een regel om in een vak met tekst, lijnen en polylijnen: breuken
(gestapeld), machten en indices (verkleind en verschoven), wortels, haken die meegroeien, matrices,
Σ/∏ met grenzen, ∫ met grenzen en dx, d/dx met evaluatiestreep, programmablokken met de verticale streep
van Mathcad, en resultaten met eenheid (`m/s²`). Haken, wortels, Σ, ∏ en ∫ worden als lijnen getekend,
zodat de weergave niet afhangt van welke symbolen een lettertype heeft. Haakjes alleen waar de voorrang
dat vraagt; een getal voor een naam of eenheid krijgt een spatie (`5 kN`), anders een `·`.

**Export en weergave** (`rb-export`). Per regio een weergave (formule, tekst, tabel, grafiek, bron met
fout); daarna paginalayout (A4 voor PDF, één doorlopende pagina voor SVG, regio's op dezelfde rij naast
elkaar) in een tussenformaat van tekenopdrachten. De SVG-schrijver bedt de lettertypen in (base64), de
PDF-schrijver is zelf geschreven: beide lettertypen als TrueType ingebed (Type0/Identity-H, ToUnicode
voor zoeken en kopiëren), streams met deflate. Grafieken met ronde schaalwaarden (1-2-5), raster,
legenda en eenheden op de assen.

**Interface** (`rb-gui`, `mathprime`). Links de bron (gewone tekst), rechts het werkblad zoals het
in de PDF komt (dezelfde tekenopdrachten, dezelfde lettertypen), met interactieve grafieken (egui_plot:
zoomen, verschuiven, waarden aflezen). Na elke toetsaanslag wordt herberekend: als precies één regel
veranderde alleen die regio en wat ervan afhangt (via de graaf), anders het hele blad; de statusbalk
toont hoeveel regio's en hoe lang. Klik op een regio en de cursor springt naar zijn regel in de bron.
Bestand: nieuw, openen, opslaan (als), exporteren naar PDF/SVG, met een vraag bij niet-opgeslagen
wijzigingen (ook bij sluiten). Sneltoetsen uit `keymap.txt` (zie §6), Ctrl+G voor Griekse letters zoals
in Mathcad, zoomen, en een hulpvenster met de syntax.

**Eenheden** (`rb-units`). Een `Quantity` is een `f64` in SI plus een `Dim` (zeven `i8`-exponenten).
Optellen van `m` en `s` is een fout, `N·m / s` is vermogen. Eenheden worden opgezocht met SI-voorvoegsels;
exacte namen gaan voor voorvoegsels (`min` is minuut, `cd` candela). Weergave: automatisch met een
voorvoegsel waar dat past (`12.5 MPa`, `1.5 kN·m`), in de eenheid die je erachter zet (`= cm^3`), of in de
eenheid die uit de definitie volgt (zie hieronder). Temperatuurschalen met een nulpunt (`°C`, `°F`) staan
apart van de gewone eenheden; `Δ°C`, `Δ°F` en `°R` zijn gewone eenheden.

**Waarden** (`rb-core/value.rs`). Een waarde is een reële grootheid, een complexe grootheid, een matrix
of een bereik. Elk matrixelement is een eigen grootheid, dus `[1 m, 2 s]` mag, net als in Mathcad.
Matrices mogen complexe elementen hebben (het imaginaire deel heeft de eenheid van het element).

**Lineaire algebra** (`rb-core/linalg.rs`). Zelf geschreven, rekent intern complex. LU met rijpivotering
voor `det`, `inverse`, `lsolve` en `lu`; Jacobi voor symmetrische eigenwaardeproblemen; Hessenberg-vorm
en QR met Wilkinson-shifts voor algemene (complexe) eigenwaarden; inverse iteratie voor eigenvectoren;
eenzijdige Jacobi voor de SVD; Householder voor QR; `polyroots` via de eigenwaarden van de
companion-matrix; FFT met radix-2 en Bluestein. Eenheden: een vierkante matrix waarmee je rekent heeft
elementen met eenheid "rij-eenheid × kolom-eenheid" (zoals een stijfheidsmatrix); de eenheden van
determinant, inverse, oplossing en eigenwaarden volgen daaruit.

**Speciale functies en verdelingen** (`rb-core/special.rs`). Lanczos voor ln Γ, reeks en kettingbreuk
voor de onvolledige gamma- en betafunctie (daarmee erf, t, χ², F, binomiaal, Poisson), trapeziumregel op
periodieke integranden voor J_n en I_n (exponentieel nauwkeurig) en Gauss-Kronrod voor Y_n en K_n.
Kwantielen door bisectie op de verdelingsfunctie; `qnorm` met Acklam plus een Halley-stap.

**Regressie en oplossers** (`rb-core/fit.rs`, `rb-core/eval/solve.rs`, `numeric.rs`). Splines,
kleinste kwadraten met Householder (`polyfit`, `linfit`), Levenberg–Marquardt (`genfit`, `Find`,
`Minerr`), Nelder–Mead met strafterm voor beperkingen (`Minimize`, `Maximize`), Brent en een secant-start
(`root`), RK4 (`rkfixed`) en Dormand–Prince 5(4) (`Rkadapt`, `odesolve`). Onbekenden worden geschaald
met hun startwaarde, zodat de oplossers in elke eenheid met getallen van orde 1 werken.

**Rekenkern** (`rb-core`). Pratt-parser met, van zwak naar sterk: `or`, `and`, `not`, vergelijkingen
(kettingen zoals `0 < x ≤ 1`), `+ -`, `* /` en juxtapositie, min vooraan, `^`, en achteraan `!` en `[i]`.
Functies zijn lazy: de body wordt pas bij de aanroep geëvalueerd, met eigen parameters en de globale
omgeving. `if` is een speciale vorm: alleen de gekozen tak wordt uitgerekend. `sum`, `prod`, `integral` en
`deriv` binden hun eigen variabele. Recursie wordt afgekapt op 200 niveaus; het blad en de REPL rekenen op
een thread met 64 MB stack, zodat die 200 niveaus ook in een debug-build passen.

**Numeriek** (`rb-core/numeric.rs`). Integralen met adaptieve Gauss-Kronrod 7/15 (QUADPACK-gewichten),
relatieve tolerantie 1e-10 met een absolute ondergrens, grenzen mogen ±∞ zijn (substitutie
x = a + t/(1−t)). Afgeleiden (orde 1 t/m 5) met centrale differenties en Richardson-extrapolatie
(methode van Ridders).

**Bereiken.** `i := 1..10` en `x := 0, 0.5 .. 3` maken een bereikvariabele. Een regel die er een gebruikt
(ook via een eigen functie) wordt per waarde uitgerekend, zoals in Mathcad: een resultaatregel geeft een
tabel met per rij een eventuele fout, `y := x^2` geeft een kolomvector (twee bereiken: een matrix), en
`v[i] := ...` vult een vector element voor element. Twee bereiken in één regel lopen alle combinaties af.

**Programmablokken.** `f(n) := {` opent een blok over meerdere regels dat eindigt met een regel `}`.
Daarbinnen zijn toekenningen lokaal (`s := 0` of `s ← 0`), en `if`/`else if`/`else`, `for k in 1..n`
(of `for k ∈ v`), `while`, `break`, `continue` en `return` werken met inspringen, zoals in Python. Zonder
`return` is de waarde van de laatste regel het resultaat. Een blok mag hoogstens 10⁷ lusdoorgangen maken.

**Werkblad** (`rb-sheet`). Eén regel is één regio; een programmablok is één regio over meerdere regels.
Een *solve block* begint met een regio `Given`, daarna beperkingen (vergelijkingen zonder `=` of `:=`,
zoals `x + y == 10` of `x > 0`), en eindigt bij de eerste regio die `Find`, `Minerr`, `Minimize`,
`Maximize` of `odesolve` gebruikt; die regio hangt in de graaf af van alle beperkingen. Elke regio heeft een positie (kolom, rij); zonder
`@kolom,rij` is dat kolom 0 van zijn regelnummer. Het blad wordt doorgerekend op volgorde van rij en dan
kolom, zoals Mathcad. De afhankelijkheidsgraaf legt voor elke regel vast welke eerdere definities hij
gebruikt (de *laatste* definitie boven hem); `v[i] := ...` hangt bovendien af van de vorige definitie van
`v`. Bij `update_region` wordt de vereniging van de oude en nieuwe afhankelijken herberekend, plus regels
waarvan de "nog niet gedefinieerd"-status veranderde. Verandert de positie, dan wordt alles opnieuw
gesorteerd en doorgerekend. Getest tegen volledig opnieuw laden (zelfde uitvoer) en tegen "alleen de
afhankelijke regio's worden herberekend".

## 3. Bewuste keuzes en afwijkingen

* **Hoeken blijven dimensieloos**, zoals in Mathcad (fase 1 heeft dit onderzocht). Een aparte hoekdimensie
  zou `sin(x)` voor gewone getallen en veel bestaande formules (`ω·t`, `v = ω·r`) breken, en de bekende
  verwarring rond `rpm` en `Hz` lost de weergave-eenheid nu grotendeels op: `n := 1450 rpm` toont als
  `1450 rpm`. Gevolg blijft: `rpm` = 2π/60 rad/s en `Hz` = 1/s, dus 1 Hz is niet 1 rpm/60.
* **Weergave-eenheid uit de definitie.** `E := 5 kJ` toont als joule (`5 kJ`), `q := 3 kN/m` als `3 kN/m`
  (in v0.1: `3000 kg/s^2`), `T := 20 °C` als `20 °C`. Dit loopt mee door optellen, aftrekken, min, en
  vermenigvuldigen of delen met een kaal getal; bij `q * L` beslist de automatische weergave weer. Heeft
  de dimensie een eigen naam (N, Pa, N·m, W), dan gaat de automatische weergave voor samengestelde
  eenheden: `1500 N*m` toont als `1.5 kN·m`. Wil je joule bij een moment: `= J`.
* **`g` is de valversnelling en `gm` de gram**, zoals in Mathcad (bevestigd in een
  [PTC-communitydraad](https://community.ptc.com/mathcad-7/kg-gm-g-as-units-5744)). `kg` en `mg` zijn expliciet.
* **Juxtapositie** is links-associatief op `*`-niveau: `1/2 x` is (1/2)·x. In Mathcad bepaalt de editor
  de structuur; in tekstinvoer moet dat via haakjes.
* **`2i` is imaginair.** Een getal direct gevolgd door `i` of `j` is een imaginair getal, zoals in Mathcad.
  Met een bereik `i` schrijf je dus `40 * i` of `40 i` (met spatie), niet `40i`. `2in` blijft 2 inch.
* **Vergelijken.** `==` vergelijkt (geeft 1 of 0), `=` blijft "toon het resultaat". `==` en `!=` hebben een
  relatieve tolerantie van 1e-12, zodat `0.1 + 0.2 == 0.3` waar is. Een kale `0` mag met elke eenheid
  vergeleken worden (`0 < L`); optellen blijft streng.
* **Complexe getallen volgen Mathcad:** `sqrt(-4)` is `2i`, `ln(-1)` is `πi` en `(-8)^(1/3)` is de
  hoofdwaarde `1 + 1.732i`. `nthroot(-8, 3)` en `cbrt(-8)` blijven reëel (−2). In v0.1 waren dit fouten.
* **`root` is Mathcad-compatibel (sinds v0.3).** `root(f(x), x)` zoekt een nulpunt vanaf de huidige waarde
  van x, `root(f(x), x, a, b)` tussen a en b (Brent). Alleen als het tweede argument een getal is
  (`root(27 m^3, 3)`) is het nog de n-de machtswortel, zoals in v0.2; schrijf daarvoor liever `nthroot`.
  Let op: `root(a, n)` met een *variabele* n zoekt nu een nulpunt van a in n.
* **Oplossers.** `Find` slaagt als de grootste relatieve restfout onder 1e-8 ligt, anders een fout met de
  restfout (gebruik dan `Minerr`). `Minimize` en `Maximize` met beperkingen gebruiken een strafterm die
  stapsgewijs wordt opgevoerd; ze zijn nauwkeurig tot ongeveer 1e-8 relatief, genoeg voor de weergave met
  6 cijfers. `genfit(vx, vy, vg, F)` verwacht F(x, u) met u de parametervector (zoals Prime, zonder
  afgeleiden). Een Given-blok werkt in een werkblad, niet in de REPL.
* **`odesolve`** lost één differentiaalvergelijking van willekeurige orde op (beginwaardeprobleem, alle
  beginvoorwaarden in hetzelfde punt) en geeft een functie die tussen begin en eind wordt geïnterpoleerd
  (1000 stappen, kubisch Hermite). Stelsels (`odesolve([x, y], t, b)`) en randwaardeproblemen nog niet;
  daarvoor is er `rkfixed`/`Rkadapt` met een vectorfunctie D(t, y).
* **FFT-normering als numpy, niet als Mathcad**: `fft(v)` is Σ v_j·e^(−2πi·jk/n) zonder factor en geeft
  het hele spectrum (n waarden, elke lengte); `ifft` deelt door n. Mathcads `fft` geeft alleen de helft
  van het spectrum voor 2^m reële waarden met een andere factor.
* **`polyfit(vx, vy, k)` in plaats van `regress`**: het geeft de coëfficiënten oplopend (c0, c1, ...),
  direct bruikbaar met `polyroots`. Mathcads `regress` geeft een vector voor `interp`.
* **Splines**: `lspline` natuurlijk (tweede afgeleide 0 aan de rand), `pspline` parabolisch,
  `cspline` met lineair doorgetrokken tweede afgeleide (exact voor kubische polynomen). De vector van
  `lspline`/`pspline`/`cspline` bevat de tweede afgeleiden in de knooppunten.
* **Toevalsgetallen per regel deterministisch.** `rnd`, `runif` en `rnorm` worden per regel gezaaid uit de
  tekst van die regel (en een variabele `SEED` als die bestaat). Daardoor geeft gedeeltelijk herrekenen
  dezelfde getallen als het hele blad opnieuw laden; twee identieke regels geven wel dezelfde getallen.
* **Functies werken element voor element op matrices** (`sqrt([4, 9])` is `[2, 3]`); in Mathcad moet je
  daarvoor vectoriseren. `abs(v)` van een vector is de lengte (zoals Mathcads |v|).
* **Matrices schrijf je als in Matlab:** `,` scheidt elementen, `;` rijen, dus `[1; 2; 3]` is een kolom
  en `[1, 2, 3]` een rij. Twee vectoren met dezelfde oriëntatie vermenigvuldigen geeft het inproduct,
  rij maal kolom ook (als getal), kolom maal rij het uitproduct.
* **`ORIGIN` is 0**, zoals in Mathcad; `ORIGIN := 1` bovenin het blad maakt indices 1-gebaseerd.
* **Variabele gaat voor eenheid én ingebouwde functie.** Een naam die bovenin als eenheid is gebruikt en
  verderop als variabele wordt gedefinieerd, krijgt een waarschuwing (in Mathcad is dat stil gedrag). Na
  `max := 5` is `max(2)` een vermenigvuldiging (10) en `max(1, 2)` een fout, net als bij eenheden.
* **Geen `nalgebra`.** Het plan noemde `nalgebra`, maar omdat elk matrixelement een eigen eenheid heeft,
  past een getallenbibliotheek slecht. In fase 2 zijn ook eigenwaarden, SVD, QR en Cholesky zelf
  geschreven (`linalg.rs`, met tests die A·v = λ·v, U·Σ·Vᵀ = A en Q·R = A controleren); de crates blijven
  zonder dependencies. Voor grote matrices (honderden rijen) is dit trager dan LAPACK; dat is pas met een
  grafische interface en echte gebruikersbladen relevant.
* **Eigenwaarden en ontbindingen met eenheden**: eigenwaarden vragen dat de diagonaal overal dezelfde
  eenheid heeft (zoals M⁻¹·K in 1/s²); SVD, QR, Cholesky, `rank` en de normen vragen één eenheid voor de
  hele matrix. Eigenvectoren zijn dimensieloos en genormeerd op lengte 1.
* **Nog niet:** symbolisch limieten, reeksen en integreren buiten veeltermen, Design of Experiments, financiële functies,
  Airy-functies (`Ai`, `Bi`), Bessel van niet-gehele orde, stelsels in `odesolve`.
* **Weergave rechtop, zoals Mathcad**: namen en eenheden staan niet cursief (Mathcad Prime toont ze
  rechtop); eenheden en resultaten krijgen wel een eigen kleur. Lettertypen: Ubuntu Light (de standaard
  van egui) met Hack als terugval voor ∈, ∧, ∨ en pijlen; interface, SVG en PDF gebruiken dezelfde
  bestanden, dus de maten kloppen overal.
* **Grafieken** zijn een eigen regio: `plot x = 0 ; 10 : sin(x), cos(x)` (200 punten),
  `plot t = 0 s ; 2 s ; 500 : u(t) = mm` (aantal punten en eenheid van de y-as), `plot vx : vy`
  (meetpunten). Een punt dat niet uit te rekenen is, onderbreekt de lijn. In de tekstweergave van de
  command-line staat alleen een samenvatting; de tekening zit in de export en de interface.
* **De interface toont de bron als tekst** en het werkblad ernaast, zoals het plan voor fase 3 voorstelde.
  Bewerken in de opgemaakte formule (de Prime-manier) is fase 4.
* **Decimale komma** wordt niet gelezen: de komma scheidt functieargumenten. Een instelling per
  taalgebied komt met de interface.

## 4. Roadmap

**Fase 1 — taal compleet maken (M). Klaar in v0.2.** Vergelijkingen en booleans (`<`, `≤`, `==`, `≠`,
`and`, `or`, `not`), `if` met meerdere takken voor stuksgewijze functies, bereiken (`i := 1..10`,
`x := 0, 0.5 .. 3`) met impliciete iteratie, vectoren en matrices (literals, indices, `v[i] :=`,
`ORIGIN`, rekenen, `det`, `inverse`, `lsolve`, `augment`, `stack`, `submatrix`, ...), sommatie en product,
numeriek differentiëren en integreren, complexe getallen, temperatuurschalen, weergave-eenheid doorgeven,
hoekdimensie onderzocht (blijft dimensieloos, zie §3), regio's met een 2D-positie. Daarnaast:
`asinh`/`acosh`/`atanh`/`acoth`, `cot`/`sec`/`csc`/`coth`, `sign`, `Φ`, `δ`, `gcd`, `lcm`, `n!`,
`combin`, `permut`, `mean`, `median`, `var`/`Var`, `stdev`/`Stdev`.
Wat uit fase 1 doorschuift: programmeerblokken (`for`, `while`; staan in `INVENTARIS.md` bij fase 1) en
complexe matrices.

**Fase 2 — functiebibliotheek en oplossers (L). Klaar in v0.3.** In deze volgorde gebouwd:
1. programmablokken (`:= {` ... `}` met `for`, `while`, `if`/`else`, `break`, `continue`, `return`);
2. complexe matrices en de rest van de lineaire algebra (`eigenvals`, `eigenvecs`, `eigenvec`, `cholesky`,
   `svd`, `svds`, `qr`, `lu`, `rank`, `cond2`, `norm1`/`norm2`/`normi`, `polyroots`, `csort`, `rsort`,
   `lookup`, `vlookup`, `match`);
3. speciale functies (`erf`, `erfc`, `Γ`, `lnGamma`, `Beta`, `Ψ`, Bessel J/Y/I/K van gehele orde) en
   verdelingen (`d`/`p`/`q` voor norm, lnorm, t, chisq, F, exp, unif, weibull; `dbinom`, `pbinom`, `dpois`,
   `ppois`), `corr`, `cvar`, `percentile`, `hist`, `rnd`, `runif`, `rnorm`;
4. interpolatie en regressie (`linterp`, `lspline`/`pspline`/`cspline` met `interp`, `slope`,
   `intercept`, `line`, `stderr`, `polyfit`, `linfit`, `genfit`);
5. oplossers (`root`, `nthroot`, solve blocks met `Given`/`Find`/`Minerr`/`Minimize`/`Maximize`);
6. differentiaalvergelijkingen (`rkfixed`, `Rkadapt`, `odesolve`) en FFT (`fft`, `ifft`, `cfft`, `icfft`).

Wat doorschuift: Design of Experiments, financiële functies en tekstfuncties (staan in `INVENTARIS.md`
bij "fase 2+"), Airy-functies, stelsels en randwaardeproblemen in `odesolve`, `regress`/`loess`, een
Given-blok in de REPL.

**Fase 3 — grafische interface (L). Klaar in v0.4.** Gebouwd zoals voorgesteld: `egui` (OpenGL-backend
`glow`, sneller te bouwen dan wgpu), tekstinvoer met de opgemaakte formules ernaast (eigen
layoutengine, `rb-layout`), grafieken (nieuwe regio `plot`, interactief met `egui_plot`, in de export zelf
getekend), export naar PDF en SVG (ook vanaf de command-line: `mathprime-cli export`), sneltoetsen uit een
configuratiebestand. De regioposities (`@kolom,rij`) bepalen ook in de weergave wat naast elkaar staat.

Wat doorschuift: regio's met de muis verslepen (posities bestaan, slepen nog niet), 3D-, pool- en
logaritmische grafieken en instelbare asgrenzen (gedaan in v1.4), direct afdrukken (nu via PDF), een
paginaweergave in de interface (het afdrukvoorbeeld van v1.4), en bewerken in de opgemaakte formule zelf (fase 4).

**Fase 4 — structuurgebaseerde formule-editor (XL).** Cursor die door breuken, machten en wortels loopt zoals
Prime, selectie uitbreiden met de spatiebalk, invoegen rond de selectie. Navigatie is gedocumenteerd
(Enter, Spatie, Home/End, pijltjes met Ctrl en Shift); de rest van de sneltoetsen haal je uit je eigen
installatie (zie §6).

**Fase 5 — symbolisch rekenen en import (XL).** Beperkte eigen CAS (differentiëren, vereenvoudigen, simpele
algebra) of een bestaande engine inbinden. `.mcdx`-import alleen na een juridische check en met een duidelijk
"best effort"-label.

## 5. Verbeteringen op Mathcad

Al gebouwd:

* tekstformaat dat in git past
* foutmeldingen met oorzaak en positie; gevolgfouten wijzen naar de bron
* herberekening via een afhankelijkheidsgraaf (alleen wat verandert)
* `deps`: waar komt een waarde vandaan, waar heeft hij invloed op; ongebruikte definities gemarkeerd
* waarschuwing bij eenheid-versus-variabele
* sweeps met foutmelding per rij, en sinds v0.2 ook resultaten over een bereik met een fout per rij
  in plaats van één rode regio
* programmablokken als gewone tekst met inspringen, zonder werkbalk of speciale sneltoetsen
* oplossers die eenheden bewaren: `Find` met een stijfheid in N/mm geeft mm, `odesolve` met
  beginwaarden in mm en m/s geeft een functie in mm, eigenwaarden van M⁻¹·K in 1/s²
* `Find` meldt de restfout als er geen oplossing is, in plaats van alleen "geen oplossing gevonden"
* weergave in de eenheid waarin je de waarde definieerde (`3 kN/m`, `1450 rpm`, `20 °C`), zonder dat je
  overal `= eenheid` hoeft te typen
* `0.1 + 0.2 == 0.3` is waar
* matrices met eenheden per element waarbij `det`, `inverse` en `lsolve` de eenheden van de uitkomst
  zelf afleiden
* de interface rekent na elke toetsaanslag alleen opnieuw wat verandert, en laat zien hoeveel
* het werkblad in de interface gebruikt dezelfde layout en lettertypen als de PDF (alleen de breedte
  volgt het venster)
* geen licentie nodig, draait overal waar Rust draait (ook Linux)

Gepland: `--watch` (herberekenen bij opslaan), parallel herberekenen van onafhankelijke regio's, tolerantie- en
foutenpropagatie als ingebouwde functie, opzoektabellen (profielen, boutmaten, staalsoorten) als gewone
`.rkn`-bibliotheken die je kunt importeren, een plugin-systeem voor eigen functies, en WebAssembly voor gebruik
in de browser.

## 6. Sneltoetsen: eerst de echte lijst ophalen

* PTC documenteert op de helppagina "Keyboard Shortcuts for Editing Math" alleen **navigatie en
  groepsuitbreiding** (Enter, Spatie, Home, End, pijltjes met Ctrl en Shift). De sneltoetsen voor het invoegen van
  operatoren staan elders, en PTC meldt dat de lijst afhangt van de toetsenbordindeling (QWERTY of QWERTZ).
* Wat ik eerder uit mijn hoofd noemde (`:` definitie, `=` evalueren, `/` breuk, `\` wortel, `[` index,
  `Ctrl+.` symbolisch) is dus **nog niet geverifieerd** voor Prime.
* Aanpak: haal de lijst uit je eigen installatie (de help of `keyboard_shortcuts.pdf` in de `Doc`-map van
  Prime, op jouw toetsenbordindeling), zet hem in een configuratiebestand, en laat de editor daaruit lezen.
  Zo zijn indelingsverschillen data en geen code, en kun je afwijkingen per persoon instellen.
* **Gebouwd in v0.4** als `keymap.txt` (in plaats van het eerder genoemde `keymap.ron`: een eenvoudig
  regelformaat `actie = toets` is leesbaar zonder extra bibliotheek). Het bestand staat in
  `%APPDATA%\MathPrime` (Windows) of `~/.config/mathprime`; Help → Sneltoetsen toont alle acties, de
  huidige toetsen en het pad, en kan een bestand met de standaardtoetsen aanmaken. Er zijn nu 13 acties
  (bestanden, export, beeld, Griekse letter, hulp); de Mathcad-sneltoetsen voor het *invoegen* van
  operatoren horen bij de formule-editor van fase 4.

## 7. Juridisch

Functionaliteit, sneltoetsen en gedrag nabouwen mag. Neem geen naam, logo, iconen, teksten, voorbeeldbladen of
bestandsformaat-specificaties van PTC over zonder te controleren wat is toegestaan. Zorg dat de bibliotheken die
je later toevoegt een licentie hebben die bij jouw doel past (bijvoorbeeld MIT of Apache-2.0).

## 8. Vragen die de volgorde bepalen

1. Wil je vooral een persoonlijk hulpmiddel voor je studie, of een volwaardige vervanger?
2. Welke onderdelen gebruik je het meest: eenheden, matrices, solve blocks, plots, programmeren?
3. Moet het ook in de browser draaien (WebAssembly), of alleen op desktop?
4. Hoe belangrijk is de mooie 2D-formuleweergave voor je eerste echt bruikbare versie?

## Bronnen

* [PTC Mathcad Help — Keyboard Shortcuts for Editing Math](https://help.ptc.com/mathcad/en/PTC_Mathcad_Help/keyboard_shortcuts_for_editing_math.html)
* [PTC Mathcad Help — Functions That Can Be Evaluated Both Numerically and Symbolically](https://help.ptc.com/mathcad/en/PTC_Mathcad_Help/functions_that_can_be_evaluated_both_num_and_symb.html)
* [PTC Mathcad Help 12.0](https://support.ptc.com/help/mathcad/r12.0/en/PTC_Mathcad_Help.html)
* [PTC-artikel CS377647 — sneltoetsenlijst Mathcad 15 versus Prime (QWERTZ-opmerking)](https://www.ptc.com/en/support/article/CS377647)
* [PTC-communitydraad — kg, gm, g als eenheden](https://community.ptc.com/mathcad-7/kg-gm-g-as-units-5744)
* QUADPACK (Piessens e.a., 1983) voor de Gauss-Kronrod-gewichten; Numerical Recipes §5.7 voor de methode
  van Ridders, §6.2/§6.4 voor de onvolledige gamma- en betafunctie, §11 voor Jacobi en `elmhes`.
* Abramowitz & Stegun, *Handbook of Mathematical Functions*, voor de referentiewaarden in de tests.
* Dormand & Prince (1980) voor de RK5(4)-coëfficiënten; Acklam voor de startbenadering van `qnorm`.
