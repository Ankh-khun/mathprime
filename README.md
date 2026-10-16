# MathPrime

Een rekenblad in Rust waarin je formules gewoon opschrijft en dat direct doorrekent, met eenheden
die meedenken. Geïnspireerd op Mathcad Prime; eigen naam, eigen bestandsformaat, geen code of
assets van PTC.

Status: **v1.7 — 3D-oppervlakken, hoogtelijnen, poolgrafieken en een tweede y-as; limieten, Taylorreeksen en meer integralen bij de pijl; Radau voor stijve vergelijkingen; regress, loess en gladmaken; direct afdrukken; sjablonen; slepen op het raster; Engelse interface; MIT-licentie; v1.6: odesolve voor stelsels en randwaardeproblemen, eenheden bij symbolisch rekenen; v1.5: melding bij een nieuwe versie, automatische tests en releases via GitHub Actions; v1.4: afdrukvoorbeeld (Ctrl+Alt+P), logaritmische assen en vaste asgrenzen in grafieken (Bodediagrammen); v1.3: één venster voor alles, automatisch bewaren met herstellen na een crash, zoeken, vervangen en hernoemen (Ctrl+F, Ctrl+H); v1.2: een nieuw uiterlijk: lint met pictogrammen zoals Mathcad Prime, echte tabbladen, een rustige statusbalk, recente bestanden op een leeg blad; v1.1: tabbladen (Ctrl+Tab), een instellingenvenster, leeg starten en verbeteringen in de bediening; v1.0: automatisch aanvullen, tabellen (Ctrl+6), inklapbare gebieden, pagina-einden en kop- en voetteksten; v0.9: symbolisch rekenen met de pijl (→) en Excel/CSV lezen en schrijven; v0.8: opslaan als Mathcad (.mcdx), tekst als waarde en een controle tegen Mathcad zelf; v0.7: formulebibliotheek en formules omwerken naar de onbekende; v0.6: Mathcad Prime-bestanden (.mcdx) openen, met afbeeldingen; v0.5: werken zoals in Mathcad Prime, met lichte en donkere modus: typen op het werkblad met de toetsen van Prime,
regio's op een pagina met raster, en een lint met alle rekenopties. Daaronder v0.4: grafische interface met opgemaakte formules, grafieken en export naar PDF
en SVG, bovenop fase 2 (programmablokken, lineaire algebra, verdelingen, regressie, solve blocks,
differentiaalvergelijkingen, FFT), fase 1 (logica, bereiken, matrices, calculus, complexe getallen,
temperaturen) en de rekenkern, eenheden, werkblad en command-line uit v0.1.**
Zie [`docs/PLAN.md`](docs/PLAN.md) voor de roadmap en [`docs/INVENTARIS.md`](docs/INVENTARIS.md) voor
wat er van Mathcad nog ontbreekt.

## Bouwen en draaien

```sh
cargo build --release          # eerste keer: egui en zijn dependencies worden opgehaald
cargo test --workspace

./target/release/mathprime                              # grafische versie (met een welkomstblad)
./target/release/mathprime examples/fase3.rkn           # een blad openen

./target/release/mathprime-cli run examples/balk.rkn
./target/release/mathprime-cli run examples/balk.rkn --watch       # automatisch herberekenen bij opslaan
./target/release/mathprime-cli run examples/fase1.rkn             # alles wat in v0.2 nieuw is
./target/release/mathprime-cli run examples/fase2.rkn             # alles wat in v0.3 nieuw is
./target/release/mathprime-cli export examples/fase3.rkn fase3.pdf   # opgemaakt, met grafieken (ook .svg)
./target/release/mathprime-cli export examples/fase3.rkn fase3.pdf --watch # export herhalen bij opslaan
./target/release/mathprime-cli run examples/fouten.rkn --strikt
./target/release/mathprime-cli deps examples/balk.rkn sigma     # wat hangt van sigma af, en waarvan sigma
./target/release/mathprime-cli deps examples/balk.rkn           # overzicht, ongebruikte definities gemarkeerd
./target/release/mathprime-cli repl                             # interactieve rekenmachine
```

De rekenkern (`rb-units`, `rb-core`, `rb-sheet`, `rb-layout`) heeft geen externe dependencies; alleen
de export (lettertypen, compressie) en de interface (egui) gebruiken crates van crates.io.

### Installeren (Windows)

```powershell
powershell -ExecutionPolicy Bypass -File tools\maak-installer.ps1
```

maakt `target\installer\MathPrime-<versie>-setup.exe` (met IExpress, dat in Windows zit). Dubbelklik
erop om te installeren, zonder beheerdersrechten en alleen voor jezelf:

- de programma's in `%LOCALAPPDATA%\Programs\MathPrime`, met de voorbeelden in `voorbeelden`;
- MathPrime in het Startmenu en op het bureaublad;
- `.rkn` opent met MathPrime; bij `.mcdx` staat MathPrime onder *Openen met* (Mathcad blijft de standaard);
- `mathprime-cli` op je PATH (nieuw terminalvenster);
- verwijderen via Instellingen → Apps → MathPrime. Je werkbladen en instellingen (`%APPDATA%\MathPrime`)
  blijven dan staan.

Een nieuwe versie installeer je door het nieuwe pakket te starten; het overschrijft de oude. Sluit
MathPrime eerst. Zonder vensters: `MathPrime-<versie>-setup.exe /Q` (de installatie meldt dan niets).

MathPrime kijkt eens per dag op GitHub of er een nieuwere versie is (alleen het versienummer, met
`curl.exe` uit Windows); is die er, dan staat rechtsonder "Versie … is uit", met een klik naar de
downloadpagina (https://github.com/Ankh-khun/mathprime/releases). Uit te zetten in Instellingen →
Updates. Zonder verbinding meldt MathPrime niets.

### Releases (GitHub Actions)

Elke push naar `main` draait de tests op Windows (`.github/workflows/ci.yml`). Een versietag maakt een
release met het installatiepakket en een zip (`release.yml`); de tag moet gelijk zijn aan de versie in
`Cargo.toml`, de notities komen uit `docs/releases/<tag>.md`:

```powershell
git tag v1.5.0; git push origin v1.5.0
```

## De grafische versie

Het werkblad werkt zoals Mathcad Prime. Je ziet A4-pagina's (met raster, ruitjes of lijnen naar
keuze, onder Document). Klik op een lege plek: daar komt het rode kruisje, en wat je typt wordt een
formule die meteen opgemaakt verschijnt. Typ je `:`, dan staat er `:=` met een invulvakje. `=` rekent
uit (typ daarna een eenheid, zoals `kN`). `/` maakt een breuk van wat ervoor staat, Spatie vergroot
de selectie (operand, uitdrukking, kant van `:=`, en als laatste de hele formule), en een punt na een
naam geeft een subscript (`M.max`). Esc, Enter of een klik ernaast
maakt de formule af, en het blad rekent opnieuw. Klik op een formule om hem te wijzigen. Sleep een
regio om hem te verplaatsen. Sleep vanaf een lege plek om regio's te selecteren (Delete, Ctrl+X/C/V);
Ctrl+Z maakt ongedaan.

Bovenin staat een lint met tabbladen, zoals in Prime en Word: Wiskunde, Operatoren, Symbolen,
Functies, Matrices, Programmeren, Eenheden, Berekenen en Document. Elke knop werkt op de formule die
je bewerkt, of begint een nieuwe op het kruisje.

| toets (Prime) | | toets (Prime) | |
|---|---|---|---|
| `:` `=` `/` `^` `\` `[` `\|` `!` | definitie, uitrekenen, breuk, macht, wortel, index, abs, faculteit | Ctrl+G | Griekse letter |
| Ctrl+M | matrix (Shift+Enter rij, Shift+Spatie kolom) | Ctrl+Shift+D / I | afgeleide, integraal |
| Ctrl+Shift+4 / 3 | som, product | Ctrl+= , Ctrl+9, Ctrl+0 | ==, ≤, ≥ |
| `]` `{` `}` | programma, lokale toekenning, if | Ctrl+Shift+] , Ctrl+; | else, else if |
| Ctrl+Shift+' , Ctrl+] | for, while | Ctrl+\ | return |
| `"` of Ctrl+T | tekst | Ctrl+2 / Ctrl+7 | grafiek, solve block |
| Ctrl+6 | tabel (Tab volgende cel, Enter rij lager) | Ctrl+. | symbolisch (→) |
| Ctrl+Shift+A | inklapbaar gebied (om de selectie) | Ctrl+Enter | pagina-einde |
| Tab, ↑/↓, Ctrl+Spatie | automatisch aanvullen | | |
| F5 | herberekenen | F2 | functies |
| Ctrl+N / O / S / P | nieuw tabblad, openen, opslaan, PDF | Ctrl+Z / Y | ongedaan, opnieuw |
| Ctrl+Tab / Ctrl+Shift+Tab | volgend / vorig tabblad | Ctrl+W | tabblad sluiten |
| Ctrl+, | instellingen | Ctrl+Shift+F1 | welkomstblad |

Typ je een naam (twee letters of meer), dan verschijnt er een lijst met passende namen: eerst die van
je blad, daarna functies en eenheden (`MP` → MPa). Tab neemt het voorstel over, ↑/↓ kiest (Enter
neemt dan het gekozen voorstel), Esc sluit de lijst; Ctrl+Spatie toont hem ook bij één letter.
Functies krijgen haakjes met de cursor ertussen. Ctrl+. geeft de symbolische pijl (→).

Toetsen van Prime voor iets wat MathPrime (nog) niet kan, geven een melding in de statusbalk.
De bron (gewone tekst) staat onder Beeld → Bron (Ctrl+Shift+B). Bewaren schrijft voor elke regio zijn
plek op het blad (`@kolom,rij`). De menutoetsen staan in `keymap.txt` en de instellingen in
`instellingen.txt` (Windows: `%APPDATA%\MathPrime`). Die stel je in via Bestand → Instellingen…
(Ctrl+,): thema (licht, donker of zoals Windows), achtergrond, leeg blad of welkomstblad bij het
starten, zoom, het aantal significante cijfers in uitkomsten en automatisch aanvullen.

MathPrime opent maar één venster: dubbelklik je in de Verkenner op een `.rkn` terwijl MathPrime al
open is, dan komt het bestand als tabblad in dat venster. Elke minuut bewaart MathPrime een kopie
van bladen met niet-opgeslagen wijzigingen (`%APPDATA%\MathPrime\herstel`); na een crash vraagt het
bij de volgende start of die terug moeten. **Ctrl+F** zoekt in het blad (Enter: volgende treffer),
**Ctrl+H** vervangt; met "Hele namen" hernoem je een variabele overal waar hij gebruikt wordt, maar
niet in stukjes van andere namen of waar dezelfde letters een eenheid zijn (de `L` van liter boven
de definitie van een variabele `L`).

**Ctrl+Alt+P** (Bestand → Afdrukvoorbeeld) toont het blad precies zoals het in de PDF komt: pagina's,
kop- en voetteksten en grafieken als vaste afbeelding. Esc gaat terug naar bewerken. **Ctrl+P** drukt
af: MathPrime maakt die PDF en geeft hem aan de PDF-lezer van Windows, die het printvenster toont
(kan die niet direct afdrukken, dan gaat de PDF open). Exporteren naar PDF is **Ctrl+Shift+P**.

Bestand → **Nieuw uit sjabloon** geeft een verslag, een tentamen (vragen als inklapbare gebieden), een
ligger op twee steunpunten of een warmtebalans, met kop- en voettekst. Regio's verslepen gaat op het
raster; valt een regio over een andere, dan schuift hij omlaag tot er plek is, en wat je sleepte blijft
geselecteerd. Instellingen → **Taal** zet de bediening in het Engels (meldingen blijven Nederlands).

Meerdere werkbladen staan in tabbladen onder het lint, zoals in Prime. Openen (of een bestand naar
het venster slepen) opent een nieuw tabblad; is het huidige blad nog leeg, dan komt het daar. Een
• op het tabblad betekent niet-opgeslagen wijzigingen; bij Ctrl+W en bij afsluiten vraagt MathPrime
per tabblad of het opgeslagen moet worden. MathPrime begint met een leeg blad; het voorbeeld staat
onder Help → Welkomstblad.

## Formulebibliotheek en omwerken

**Formules** (op het lint onder Wiskunde) opent rechts een bibliotheek met ruim 75 formules:
mechanica, sterkteleer, vloeistoffen, thermodynamica, elektrotechniek, meetkunde en windenergie.
Zoek op naam of grootheid. Klik bij een formule op de grootheid die je zoekt ("Oplossen naar: σ M W").
MathPrime zet de formule dan omgewerkt op het blad (`W := M/σ =`), met daarboven een invoerregel
(waarde 1, met eenheid) voor elke grootheid die het blad nog niet kent. Met **+ Eigen formule
opslaan** bewaar je je eigen formules; die staan in `formules.txt` in de instellingenmap.

**Omwerken naar ▾** (ook op het lint) werkt de formule waarin je staat om naar een andere
grootheid. Staat die er één keer in, dan schrijft MathPrime de formule om (+ ↔ −, · ↔ /,
xⁿ ↔ ⁿ√, sin ↔ asin, ln ↔ exp). Staat hij er vaker in, dan komt er een solve block
(Given/Find) met een beginwaarde. Elke formule uit de bibliotheek is in de tests naar elke grootheid
omgewerkt en doorgerekend, met eenheden.
## Mathcad-bestanden openen

Bestand → Openen kan ook een Mathcad Prime-bestand (`.mcdx`) openen, of je sleept het naar het
venster. Formules, tekst, koppen, grafieken, solve blocks en afbeeldingen (PNG en BMP) komen mee, elk
op zijn plek. Ook de rekeninstellingen van het blad komen mee: een afwijkende eerste index
(`ORIGIN`) en het zaad van de toevalsgetallen (`SEED`); een ander eenhedenstelsel of andere
solver-toleranties worden gemeld. Wat niet overkomt, staat als grijze notitie op die plek en in een lijstje. Het
.mcdx-bestand zelf blijft ongewijzigd: opslaan maakt een `.rkn`. Op de command-line:

```text
mathprime-cli import tentamen.mcdx tentamen.rkn
mathprime-cli run tentamen.mcdx          # ook direct
```

Getest op 11 echte bladen: alle 194 uitkomsten die Mathcad in die bestanden bewaarde (ook die met
een eenheid, in SI, en de foutmeldingen), komen uit MathPrime precies zo
(`cargo test -p rb-mcdx -- --nocapture` toont het overzicht).

### Opslaan als Mathcad

Bestand → **Opslaan als Mathcad (.mcdx)…** (of `mathprime-cli export blad.rkn blad.mcdx`) schrijft
een bestand dat Mathcad Prime opent en doorrekent: formules, tekst, koppen, afbeeldingen, grafieken
en solve blocks. Wat Mathcad niet kent (programmablokken, sweeps), komt er als tekst in, met een
melding. Je eigen `.rkn` blijft het werkbestand. Heen en terug (.mcdx → MathPrime → .mcdx) geeft
voor alle 11 echte bladen dezelfde uitkomsten.

### Vergelijken met Mathcad

`mathprime-cli vergelijk blad.mcdx` (of een map) zet de uitkomsten die Mathcad in een .mcdx bewaarde
naast die van MathPrime. `tools/mathcad-vergelijk.ps1` doet de hele keten met je eigen Mathcad
Prime, via de officiële Automation API: MathPrime slaat de testbladen uit `tests/corpus` op als
.mcdx, Mathcad rekent ze door en slaat ze op, en `vergelijk` maakt een rapport. Huidige stand:
49 van 49 gelijk (twee uitkomsten rekent de gratis Mathcad Express niet uit: `lsolve` en solve
blocks).

## Symbolisch rekenen (→)

Typ een uitdrukking en druk op **Ctrl+.** (of typ `->`). Daarachter kan een sleutelwoord staan; de
knoppen staan op het lint onder Operatoren → Symbolisch.

| regel | uitkomst |
|---|---|
| `(x + 1)^2 → expand` | x² + 2x + 1 |
| `x^2 - 5*x + 6 → factor` | (x − 3)(x − 2) |
| `x^2 - 4 == 0 → solve, x` | [−2; 2] |
| `a*x^2 + b → substitute, x = 2` | 4a + b |
| `2*x + 3*x → simplify` | 5x |
| `deriv(x^3 + 2*x, x, x) →` | 3x² + 2 |
| `integral(t, t, 0, x) → simplify` | x²/2 |
| `integral(x * exp(x), x, 0, 1) →` | 1 (partieel; ook sin, cos, tan, exp, sinh, cosh en machten van a·x + b) |
| `sin(x) / x → limit, x = 0` | 1 (l'Hôpital; ook `x = ∞`) |
| `cos(x) → series, x = 0, 5` | x⁴/24 − x²/2 + 1 (Taylor tot en met de macht 4) |
| `sqrt(2) * x → float, 4` | 1.414 x |

Namen met een bekende waarde worden ingevuld en eigen functies uitgeschreven, zoals in Mathcad. Een
waarde met eenheid komt erin als getal maal eenheid, en eenheden zijn daarna gewone symbolen
(`F * x → 10 kN · x` met `F := 10 kN`). Blijven er alleen getallen en eenheden over, dan wordt dat één
grootheid: `F * a → 20 kN · m`, `1 m + 50 cm → 1.5 m`, `k * u == 10 N → solve, u` geeft `5 mm`. Het rekenwerk is
eigen code (`rb-core/src/symbolic.rs`): veeltermen met exacte breuken, wortels vereenvoudigen,
differentiëren, integreren van veeltermen, de abc-formule. In .mcdx staat een pijlregel als
`ml:symEval`; heen en terug via Mathcad Prime blijft het sleutelwoord met argument bewaard. De
gratis Mathcad Express rekent symbolisch niet uit (premium), dus vergelijken kan alleen met een
volledige licentie.

## Het werkbladformaat (`.rkn`)

Gewone tekst, één regio per regel, dus prima te vergelijken en samen te voegen in git.

```text
# Kop
" Tekstregel (begint met een aanhalingsteken, zoals in Mathcad)
// commentaar

L := 2 m                   definitie
q := 3 kN/m   // toelichting
f(x, y) := x^2 + y         functie

q * L =                    resultaat, eenheid automatisch
q * L = kN                 resultaat in een gekozen eenheid
q =                        toont 3 kN/m: de eenheid uit de definitie loopt mee
M.max := q * L^2 / 8       namen mogen een letterlijk subscript hebben (M.max) en unicode (σ, Δ)
M := q * L^2 / 8 = kN*m    definiëren en meteen tonen, zoals in Mathcad
I := 230 V / 50 Ω =        naast elkaar bindt sterker dan * en /: (230 V)/(50 Ω);
                           schrijf ½ m v² dus als 1/2 * m * v^2
P == F * v                 een formule die alleen getoond wordt (uitleg)
s := concat("Hallo ", "wereld")   tekst als waarde (ook strlen, substr, search, num2str, str2num)
M := READCSV("meting.csv")  bestand lezen (ook READEXCEL("proef.xlsx", "Blad1!A2:C20"), READFILE)
WRITEEXCEL("uit.xlsx", M) =  schrijven (ook WRITECSV, WRITEFILE); paden zijn relatief aan het blad
image 200x120 data:image/png;base64,...   een afbeelding (in het bestand zelf)
tabel x [mm], F [kN] {     tabel (Ctrl+6): elke kolom wordt een vector met eenheid,
    0, 0                   hier x := [0; 10] * (mm) en F := [0; 2.5] * (kN)
    10, 2.5
}
area Invoer                een inklapbaar gebied tot `area end`; ingeklapt: `area ingeklapt Invoer`
area end                   (de inhoud rekent mee maar is niet te zien; klik op ▾ om in te klappen)
pagebreak                  wat eronder staat, begint op een nieuwe pagina
header Verslag {bestand}   kop- en voettekst op elke pagina, met {pagina}, {paginas}, {bestand}, {datum}
footer pagina {pagina} van {paginas}
(x + 1)^2 → expand         symbolisch (Ctrl+. of ->): x^2 + 2 * x + 1
x^2 - 4 == 0 → solve, x    ook simplify, factor en substitute, x = 2

sweep hh = 80 mm ; 160 mm ; 5 : 6 * M.max / (b * hh^2) = MPa    parametersweep, 5 punten
plot x = 0 m ; L : w(x), v(x) = mm    grafiek van functies (200 punten), y-as in mm
plot t = 0 s ; 2 s ; 500 : u(t)       met een gekozen aantal punten
plot x.m : F.m                        meetpunten (twee vectoren)
plot f = 10 Hz ; 100 kHz : abs(H(f)) | log xy    logaritmische assen (Bodediagram)
plot x = 0 ; 10 : sin(x) | y = -1 ; 1, x = 2 ; 8  vaste asgrenzen (zonder eenheid: in die van de as)
plot θ = 0 ; 2*π : 1 + cos(θ) | polar             poolgrafiek (x en y even groot)
plot t = 0 s ; 10 s : F(t) ; u(t) = kN ; mm        na ';' de rechter y-as, met eigen eenheid
plot x = -3 ; 3, y = -3 ; 3 : f(x, y)             3D-oppervlak (slepen draait; 40 × 40 punten)
plot x = -3 ; 3, y = -3 ; 3 : f(x, y) | contour   hoogtelijnen
@40,3 b := 2 m             regio op kolom 40, rij 3 (volgorde: rij, dan kolom)
```

### Rekenen

```text
M(x) := if(x <= a, R.A * x, R.A * x - F * (x - a))     stuksgewijs; ook if(v1, w1, v2, w2, ..., anders)
L > 3 m and F < 20 kN =                               vergelijken en logica geven 1 of 0
0 < x ≤ 1 =                                           ketting van vergelijkingen; ook ==, !=, ≠, not, or

x := 0 m, 1 m .. 4 m                                  bereik met stap (i := 1..10: stap 1)
M(x) = kN*m                                           tabel: één rij per waarde van x
v := x^2                                              definitie over een bereik: een kolomvector
h[i] := (80 + 40 * i) mm                              element voor element, zoals in Mathcad

K := [300, -100; -100, 100] N/mm                      matrix: ',' tussen elementen, ';' tussen rijen
u := lsolve(K, [0; 1.5] kN)                           eenheden van de uitkomst volgen vanzelf
K^-1 = mm/N                                           ook det(K), transpose(K), K[0, 1]; ORIGIN := 1 voor 1-gebaseerd

sum(1/k^2, k, 1, 100) =                               ook Σ, prod en ∏; sum(v) telt een vector op
integral(M(x), x, 0 m, L) = kN*m^2                    ook ∫ en grenzen ±∞
deriv(M(x), x, 2 m) = kN                              deriv(f, x, punt, orde) tot orde 5

Z := R + 1i * ω * L.s + 1 / (1i * ω * C)              complex: 2i of 2j; Re, Im, arg, conj, abs
T := 20 °C                                            temperatuur met nulpunt (ook °F, degC, degF)
T + 5 Δ°C = °F                                        Δ°C en Δ°F voor verschillen
```

### Programmeren en oplossen

```text
stappen(n) := {                 programmablok: inspringen zoals in Python, afsluiten met }
    k := 0                      toekenning binnen een blok is lokaal (ook k ← 0)
    while n != 1                ook: for k in 1..n, for k ∈ v, if / else if / else,
        if mod(n, 2) == 0            break, continue, return
            n := n / 2
        else
            n := 3 * n + 1
        k := k + 1
    k                           zonder return: de waarde van de laatste regel
}

root(tan(z) - z, z, 4, 4.6) =   nulpunt tussen grenzen; root(f(x), x) start bij de huidige x
polyroots([-6, 11, -6, 1]) =    nulpunten van -6 + 11x - 6x² + x³ (coëfficiënten oplopend)

F.1 := 1 kN                     startwaarden boven het blok
F.2 := 1 kN
Given                           solve block
F.1 + F.2 == 10 kN              beperkingen: ==, <, >, ≤, ≥
F.1 * 2 m == F.2 * 3 m
Find(F.1, F.2) = kN             ook Minerr(x, ...), Minimize(f, x, ...), Maximize(f, x, ...)

Given
u''(t) + 2 ζ ω u'(t) + ω^2 u(t) == 0
u(0 s) == 10 mm
u'(0 s) == 0 m/s
u := odesolve(t, 2 s)           een functie: u(1 s) = mm
                                stelsel: x'(t) == y(t), y'(t) == -x(t), ... en dan
                                [x; y] := odesolve([x; y], t, 10)
                                randwaarden: w''''(x) == q/EI, w(0 m) == 0 m, w''(0 m) == 0,
                                w(L) == 0 m, w''(L) == 0, w := odesolve(x, L)   (schietmethode)
rkfixed([1; 0], 0, π, 100, D) = matrix met t en y; D(t, y) := [y[1]; -y[0]]  (ook Rkadapt, en Radau voor stijve vergelijkingen)

λ := eigenvals(M^-1 * K)        ook eigenvecs, svd, svds, qr, lu, cholesky, rank, cond2
slope(x.m, F.m) = N/mm          ook intercept, line, stderr, polyfit, linfit(vx, vy, F), genfit
interp(regress(vx, vy, 2), vx, vy, x) =   veelterm; ook interp(loess(vx, vy, 0.5), vx, vy, x)
ksmooth(vx, vy, b)  medsmooth(vy, 5)       gladmaken; localmax(vy), localmin(vy) geven indices; filterNaN(M)
interp(cspline(vx, vy), vx, vy, x) =    splines; linterp(vx, vy, x) lineair
qnorm(0.975, 10 mm, 2 mm) =     verdelingen d/p/q: norm, lnorm, t, chisq, F, exp, unif, weibull
fft(v) =                        ook ifft; elke lengte
```

* Van boven naar beneden (en op een rij van links naar rechts): een naam moet vóór de plek van gebruik
  gedefinieerd zijn.
* `5 kN`, `2 pi r`, `2(x+1)`: impliciet vermenigvuldigen. Dit staat op dezelfde hoogte als `*` en `/`
  (links-associatief), dus `1/2 x` is (1/2)·x. Gebruik haakjes voor een noemer: `1/(2 x)`.
* `2i` is een imaginair getal. Met een bereikvariabele `i` schrijf je `40 * i` of `40 i`, niet `40i`.
* Een variabele gaat voor een eenheid of ingebouwde functie met dezelfde naam (`m := 5` maakt van `m`
  een getal).
* Hoeken zijn dimensieloos: `sin(30 deg)` werkt, en `rpm` is `rev/min = 2π/60 rad/s`.
* `g` is de valversnelling en `gm` een gram, zoals in Mathcad.

Eenheden: SI-basis en afgeleide eenheden met voorvoegsels (`kN`, `MPa`, `µm`, `kWh`, ...), plus `in`, `ft`,
`lb`, `psi`, `bar`, `hp`, `PS`, `L`, `rpm`, `deg`, ... en temperaturen `°C`, `°F`, `Δ°C`, `Δ°F`, `°R`.
De volledige tabel staat in `crates/rb-units/src/table.rs`.

Ingebouwde functies (`:functies` in de REPL toont ze allemaal):

| groep | functies |
|---|---|
| wortels, machten, log | `sqrt cbrt nthroot exp ln log log2` |
| goniometrie | `sin cos tan cot sec csc asin acos atan acot atan2 sinh cosh tanh coth asinh acosh atanh acoth` |
| afronden en overig | `abs sign floor ceil trunc round mod hypot min max Φ δ gcd lcm factorial (n!) combin permut` |
| complex | `Re Im arg conj` |
| speciale functies | `erf erfc Γ lnGamma Beta Ψ J0 J1 Jn Y0 Y1 Yn I0 I1 In K0 K1 Kn` |
| statistiek | `sum prod mean median var Var stdev Stdev corr cvar percentile hist` |
| verdelingen | `dnorm pnorm qnorm` (en lnorm, t, chisq, F, exp, unif, weibull), `dbinom pbinom dpois ppois rnd runif rnorm` |
| vectoren en matrices | `rows cols length last transpose det inverse identity augment stack submatrix diag tr lsolve dot cross norm sort csort rsort reverse match lookup vlookup` |
| lineaire algebra | `eigenvals eigenvecs eigenvec cholesky svd svds qr lu rank cond2 norm1 norm2 normi norme polyroots fft ifft` |
| interpolatie en regressie | `linterp lspline pspline cspline interp slope intercept line stderr polyfit linfit genfit` |
| oplossers en ODE's | `root Find Minerr Minimize Maximize rkfixed Rkadapt odesolve` |
| speciale vormen | `if sum/Σ prod/∏ integral/∫ deriv` |

Functies van één getal werken ook element voor element op een vector of matrix (`sqrt([4, 9])`).

## Indeling

| crate | taak |
|---|---|
| `rb-units` | dimensies, eenheden, grootheden, temperatuurschalen, weergave (`12.5 MPa`, `1.5 kN·m`) |
| `rb-core` | lexer, parser (Pratt), waarden (complex, matrix, bereik), evaluator, ingebouwde functies, lineaire algebra, speciale functies en verdelingen, regressie, oplossers, ODE's, FFT |
| `rb-sheet` | werkbladformaat, posities, programmablokken, solve blocks, grafiekregio's, afhankelijkheidsgraaf, herberekening, sweeps |
| `rb-layout` | formuleweergave: breuken, machten, wortels, matrices, Σ/∏/∫ als vakken met tekst en lijnen |
| `rb-export` | weergave per regio, paginalayout, grafieken tekenen, SVG en PDF (lettertypen ingebed) |
| `rb-cli` | binary `mathprime-cli` (run, deps, repl, export) |
| `rb-mcdx` | Mathcad Prime-bestanden inlezen (eigen ZIP- en XML-lezer) |
| `rb-edit` | de formule-editor: bewerkbare formule, gedrag van de toetsen van Prime, opmaak met cursor |
| `rb-gui` | binary `mathprime` (egui): werkblad-canvas, lint, interactieve grafieken, bestanden |

## Wat MathPrime al beter doet dan het origineel

* Tekstformaat dat in git past.
* Foutmeldingen die zeggen wat er misgaat ("eenheden komen niet overeen bij optellen: m en s"), met een
  caret bij schrijffouten, en gevolgfouten die naar de oorzaak wijzen in plaats van overal rood te worden.
* Herberekening via een afhankelijkheidsgraaf: bij een wijziging worden alleen de regels herberekend die
  ervan afhangen (getest, zie `crates/rb-sheet/tests/sheet.rs`).
* `deps`: zie waar een waarde vandaan komt en waar hij invloed op heeft; ongebruikte definities worden gemarkeerd.
* Waarschuwing als een naam bovenin als eenheid geldt maar onderin een variabele wordt.
* Sweeps en resultaten over een bereik met per rij de foutmelding in plaats van een mislukte hele berekening.
* Een waarde verschijnt in de eenheid waarin je hem definieerde (`3 kN/m`, `1450 rpm`, `20 °C`).
* `0.1 + 0.2 == 0.3` is waar.
* `det`, `inverse` en `lsolve` leiden de eenheden van de uitkomst zelf af.
* Oplossers bewaren eenheden: `Find` geeft mm als de onbekende in mm begon, `odesolve` geeft een functie in
  de eenheid van de beginwaarde, eigenwaarden van M⁻¹·K komen in 1/s².
* `Find` noemt de restfout als er geen oplossing is.

## Licenties

MathPrime zelf valt onder de MIT-licentie (`LICENSE`): gebruiken, aanpassen en verspreiden mag, met
de copyrightregel erbij. Meehelpen: zie `CONTRIBUTING.md`.

De pictogrammen in de interface zijn [Tabler Icons](https://tabler.io/icons) (outline, versie 3.31.0),
MIT-licentie, © Paweł Kuna; het lettertype staat in `crates/rb-gui/assets/tabler-icons-outline.ttf`
met de licentie ernaast (`tabler-icons-LICENSE.txt`).
