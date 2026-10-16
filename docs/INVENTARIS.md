# Functie-inventaris: Mathcad Prime versus MathPrime

Doel: afvinken wat er van Mathcad nog ontbreekt. Per categorie: voorbeelden, moeilijkheid en fase
(zie `PLAN.md`).

**Kolom "bron"**: *PTC-help* = categorie en voorbeelden staan in de PTC-documentatie die ik heb ingezien;
*eigen kennis* = uit mijn kennis van Mathcad en nog te controleren tegen jouw installatie of de PTC-help.
Dit is een planningslijst, geen volledige functielijst: de definitieve namen en argumenten haal je uit de
officiële help.

Moeilijkheid: **S** uren, **M** dagen, **L** weken, **XL** maanden (relatief, afhankelijk van beschikbare tijd).

## Aanwezig

| onderdeel | wat | sinds |
|---|---|---|
| Operatoren | `+ - * / ^`, impliciet vermenigvuldigen, min vooraan, `n!` | v0.1, `!` v0.2 |
| Wortels en machten | `sqrt cbrt nthroot`, gebroken machten als de eenheid het toelaat | v0.1, `nthroot` v0.3 |
| Goniometrisch | `sin cos tan cot sec csc asin acos atan acot atan2` (hoeken dimensieloos, `deg` en `rad`) | v0.1, rest v0.2 |
| Hyperbolisch | `sinh cosh tanh coth asinh acosh atanh acoth` | v0.1, rest v0.2 |
| Log en exponentieel | `exp ln log log2` | v0.1 |
| Afronden | `floor ceil trunc round` | v0.1 |
| Overig | `abs min max hypot mod sign` | v0.1, `sign` v0.2 |
| Eenheden | SI + veelgebruikte niet-SI, voorvoegsels, dimensieanalyse, weergave in gekozen eenheid | v0.1 |
| Weergave-eenheid uit de definitie | `E := 5 kJ` toont joule, `q := 3 kN/m` toont `kN/m` | v0.2 |
| Temperatuurschalen | `°C`, `°F` (ook `degC`, `degF`) met nulpunt; `Δ°C`, `Δ°F`, `°R`; `= °C` | v0.2 |
| Vergelijken en logica | `< ≤ > ≥ == ≠` (ook `<= >= !=`), kettingen `0 < x ≤ 1`, `and or not` (ook `∧ ∨ ¬`) | v0.2 |
| Stuksgewijs | `if(v, a, b)`, `if(v1, a1, v2, a2, ..., anders)`, `Φ` (Heaviside), `δ` (Kronecker) | v0.2 |
| Bereiken | `i := 1..10`, `x := 0, 0.5 .. 3`, impliciete iteratie, tabel per waarde | v0.2 |
| Programmeerblokken | `f(n) := {` ... `}` met lokale toekenning (`:=`, `←`), `if`/`else if`/`else`, `for k in 1..n`, `for k ∈ v`, `while`, `break`, `continue`, `return` | v0.3 |
| Vector en matrix | `[1, 2; 3, 4]`, `v[i]`, `M[i, j]`, `v[i] := ...`, `ORIGIN`, `+ - *`, `M^n`, `M^-1`, complexe elementen | v0.2, complex v0.3 |
| Matrixfuncties | `rows cols length last transpose det inverse identity augment stack submatrix diag tr lsolve dot cross norm sort reverse` | v0.2 |
| Lineaire algebra | `eigenvals eigenvecs eigenvec cholesky svd svds qr lu rank cond2 norm1 norm2 normi norme` | v0.3 |
| Zoeken en sorteren | `csort rsort match lookup vlookup` | v0.3 |
| Sommatie en product | `sum(f, i, a, b)` / `Σ`, `prod(f, i, a, b)` / `∏`, `sum(v)`, `prod(v)` | v0.2 |
| Calculus (numeriek) | `integral(f, x, a, b)` / `∫` (ook met ±∞), `deriv(f, x, punt[, orde])` | v0.2 |
| Complexe getallen | `2i`, `3j`, `Re Im arg conj abs`, complexe `sqrt ln exp ^` | v0.2 |
| Getaltheorie en combinatoriek | `gcd lcm factorial combin permut` | v0.2 |
| Statistiek | `mean median var Var stdev Stdev corr cvar percentile hist` | v0.2, rest v0.3 |
| Verdelingen | `d`/`p`/`q` + `norm lnorm t chisq F exp unif weibull`; `dbinom pbinom dpois ppois`; `rnd runif rnorm` | v0.3 |
| Speciale functies | `erf erfc Γ (gamma) lnGamma Beta Ψ (digamma)`, Bessel `J0 J1 Jn Y0 Y1 Yn I0 I1 In K0 K1 Kn` | v0.3 |
| Interpolatie | `linterp`, `lspline`/`pspline`/`cspline` + `interp` | v0.3 |
| Regressie | `slope intercept line stderr polyfit linfit genfit` | v0.3 |
| Oplossen | `root(f, x)`, `root(f, x, a, b)`, `polyroots`, `lsolve`, `Given` met `Find`/`Minerr`/`Minimize`/`Maximize` | v0.3 |
| Differentiaalvergelijkingen | `rkfixed`, `Rkadapt`, `odesolve` (willekeurige orde; stelsels `[x; y] := odesolve([x; y], t, b)`; begin- en randwaardeproblemen) | v0.3, v1.6 |
| Signaalverwerking | `fft ifft cfft icfft` (elke lengte; numpy-normering, zie `PLAN.md` §3) | v0.3 |
| Werkblad | definities, functies, resultaten, tekst, koppen, sweeps, posities `@kolom,rij`, afhankelijkheidsgraaf, meerregelige blokken, solve blocks | v0.1, posities v0.2, blokken v0.3 |
| Grafieken (2D) | `plot x = a ; b [; n] : f(x), g(x) [= eenheid]`, `plot vx : vy` (meetpunten), legenda, eenheden op de assen, interactief in de interface; logaritmische assen en vaste asgrenzen (`\| log xy, y = a ; b`) | v0.4, v1.4 |
| Formuleweergave | breuken, machten, indices, wortels, haken, matrices, Σ/∏/∫, d/dx, programmablokken, eenheden in resultaten | v0.4 |
| Export | PDF (A4, ingebedde lettertypen) en SVG, ook via `mathprime-cli export` | v0.4 |
| Tekst | strings als waarde; `concat strlen substr search num2str str2num str2vec vec2str IsString error` | v0.8 |
| Bestanden | `READCSV READEXCEL READFILE WRITECSV WRITEEXCEL WRITEFILE` (.csv, .prn, .txt, .xlsx) | v0.9 |
| Symbolisch | `→` met `simplify expand factor solve substitute`; `deriv` en veeltermintegralen symbolisch (geen limieten, reeksen, `float`, `assume`) | v0.9 |

## Nog te bouwen

| categorie | voorbeelden | moeilijkheid | fase | bron |
|---|---|---|---|---|
| Differentiaalvergelijkingen (rest) | `sbval`, `bvalfit` (randwaarden zonder Given-blok), stijve oplossers (`Stiffb`, `Radau`), `Jacob` | M | 2+ | PTC-help (Jacob); overige eigen kennis |
| Speciale functies (rest) | Airy `Ai`, `Bi`, Bessel van niet-gehele orde, elliptische integralen | M | 2+ | PTC-help |
| Data-analyse (rest) | `regress`, `loess`, lokaal minimum/maximum, uitschieters en NaN, PCA (`Nipals`), smoothing | L | 2+ | PTC-help |
| Design of Experiments | ontwerpmatrices, factor screening, ANOVA, regressie, Monte Carlo | L | 2+ | PTC-help |
| Financieel | contante waarde, toekomstige waarde, rente, betaling, intern rendement | S–M | 2+ | PTC-help |
| Grafieken (rest) | 3D-plots, poolgrafieken, `CreateSpace`/`CreateMesh` | L | 3+ | PTC-help (Graphing) |
| Beeldverwerking | kleur, binarisatie, convolutie, morfologie, transformaties | L | 5+ | PTC-help; waarschijnlijk laag prioriteit |

## Interface en bediening

| onderdeel | moeilijkheid | fase | opmerking |
|---|---|---|---|
| 2D-formuleweergave (breuken, wortels, indices) | L | 3 | **klaar in v0.4** (`rb-layout`) |
| Grafische interface | L | 3 | **klaar in v0.4** (`mathprime`); **v0.5**: typen op het werkblad zoals in Prime, slepen, selecteren, ongedaan maken, lint met tabbladen, achtergrond met raster/ruitjes/lijnen |
| Vrij te plaatsen regio's op een canvas | M | 3+ | posities bestaan (`@kolom,rij`) en worden getoond; slepen met de muis nog niet |
| Structuurgebaseerde formule-editor | XL | 4 | cursorgedrag, selectie uitbreiden, invoegen rond selectie |
| Sneltoetsen | M | 3–4 | **v0.5**: de toetsen van Prime 10 op het werkblad (`rb-edit/src/keys.rs`); `keymap.txt` voor de menu-acties |
| PDF-export | M | 3 | **klaar in v0.4** (en SVG) |
| `.mcdx`-import | L | 5 | juridisch eerst checken; nooit volledig |
| Formulebibliotheek en omwerken naar de onbekende | M | — | **v0.7** (eigen uitbreiding; Mathcad heeft symbolisch `solve`) |
| Tekst als waarde (strings) | M | — | **v0.8** (concat, strlen, substr, search, num2str, str2num, str2vec, vec2str) |
| Opslaan als Mathcad (.mcdx) | L | — | **v0.8** (heen en terug gelijk voor 11 echte bladen; Mathcad opent en rekent ze door) |
| Controle tegen Mathcad zelf | M | — | **v0.8** (`tools/mathcad-vergelijk.ps1` + `mathprime-cli vergelijk`: 49/49 gelijk) |
| Automatisch aanvullen | M | — | **v1.0** (namen van het blad, functies en eenheden; Tab, ↑↓, Ctrl+Spatie; `rb-edit/src/complete.rs`) |
| Tabellen (Ctrl+6) | M | — | **v1.0** (`tabel x [mm], F [kN] { ... }`: kolommen met eenheid; celbewerker met Tab en Enter; naar .mcdx als een definitie per kolom) |
| Inklapbare gebieden, pagina-einden, kop- en voetteksten | M | — | **v1.0** (`area` … `area end`, `pagebreak`, `header`/`footer` met {pagina} {paginas} {bestand} {datum}; Ctrl+Shift+A, Ctrl+Enter; pagina-einde ook in .mcdx) |
