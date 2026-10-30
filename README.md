# MathPrime

*[Nederlandse versie: README.nl.md](README.nl.md)*

A worksheet calculator written in Rust: you write formulas the way you would on paper, they calculate
immediately, and units are checked along the way. Inspired by Mathcad Prime, with its own name and its own
file format, and no code or assets from PTC. It opens and saves `.mcdx` files too.

**Download:** [latest release](https://github.com/Ankh-khun/mathprime/releases/latest) (Windows installer or zip).
Licensed under MIT. Internal code name: *Justice*.

See [`docs/ROADMAP.md`](docs/ROADMAP.md) for what is planned, [`docs/INVENTORY.md`](docs/INVENTORY.md) for what is
still missing compared to Mathcad, and [`docs/releases/`](docs/releases) for the history. The design notes in
`docs/PLAN.md` are in Dutch.

## Building and running

```sh
cargo build --release          # first time: downloads egui and its dependencies
cargo test --workspace

./target/release/mathprime                              # graphical version
./target/release/mathprime examples/fase3.rkn           # open a sheet

./target/release/mathprime-cli run examples/balk.rkn
./target/release/mathprime-cli run examples/balk.rkn --watch       # recalculate on every save
./target/release/mathprime-cli export examples/fase3.rkn fase3.pdf   # typeset, with plots (also .svg)
./target/release/mathprime-cli run examples/fouten.rkn --strikt
./target/release/mathprime-cli deps examples/balk.rkn sigma     # what depends on sigma, and what sigma uses
./target/release/mathprime-cli deps examples/balk.rkn           # overview, unused definitions marked
./target/release/mathprime-cli repl                             # interactive calculator
```

The calculation core (`rb-units`, `rb-core`, `rb-sheet`, `rb-layout`) has no external dependencies; only the
export (fonts, compression) and the interface (egui) use crates from crates.io. The command line speaks Dutch;
the graphical version speaks Dutch and English.

### Installing (Windows)

Download `MathPrime-<version>-setup.exe`, or build it yourself:

```powershell
powershell -ExecutionPolicy Bypass -File tools\maak-installer.ps1
```

That makes `target\installer\MathPrime-<version>-setup.exe`: the installer (`crates/rb-setup`) with the program
files appended. Double-click it; a window asks for the install folder and whether you want a desktop shortcut, and
installs for you alone, without administrator rights:

- the programs in `%LOCALAPPDATA%\Programs\MathPrime`, with the examples in `voorbeelden`;
- MathPrime in the Start menu and on the desktop;
- `.rkn` opens with MathPrime; for `.mcdx` MathPrime is listed under *Open with* (Mathcad stays the default);
- `mathprime-cli` on your PATH (new terminal window);
- uninstall via Settings → Apps → MathPrime. Your worksheets and settings (`%APPDATA%\MathPrime`) are kept.

Without a window: `MathPrime-<version>-setup.exe /Q` (with `--doel <folder>` for another folder).

MathPrime checks GitHub once a day for a newer version (only the version number, with `curl.exe` from Windows); if
there is one, "Version … is out" appears at the bottom right. You can check yourself via Help → Check for updates or
Settings → Updates, which also shows your version. **Download and install** fetches the new setup, asks to save open
work as usual, closes MathPrime, installs and starts the new version. The daily check can be turned off in
Settings → Updates; without a connection MathPrime says nothing.

### Releases (GitHub Actions)

Every push to `main` runs the tests on Windows (`.github/workflows/ci.yml`). A version tag makes a release with the
installer and a zip (`release.yml`); the tag must equal the version in `Cargo.toml`, the notes come from
`docs/releases/<tag>.md`:

```powershell
git tag v1.9.0; git push origin v1.9.0
```

## The graphical version

The worksheet works like Mathcad Prime. You see A4 pages (with dots, squares or lines, under Document). Click an
empty spot: the red cross appears there, and what you type becomes a formula that is typeset right away. Type `:`
and you get `:=` with a placeholder. `=` calculates (then type a unit, such as `kN`). `/` makes a fraction of what is
before it, Space grows the selection (operand, expression, side of `:=`, and finally the whole formula), and a dot
after a name gives a subscript (`M.max`). Esc, Enter or a click elsewhere finishes the formula and the sheet
recalculates. Click a formula to change it. Drag a region to move it. Drag from an empty spot to select regions
(Delete, Ctrl+X/C/V); Ctrl+Z undoes.

At the top is a ribbon with tabs, as in Prime and Word: Math, Operators, Symbols, Functions, Matrices, Programming,
Units, Calculate and Document. Every button works on the formula you are editing, or starts a new one at the cross.

| key (Prime) | | key (Prime) | |
|---|---|---|---|
| `:` `=` `/` `^` `\` `[` `\|` `!` | definition, evaluate, fraction, power, root, index, abs, factorial | Ctrl+G | Greek letter |
| Ctrl+M | matrix (Shift+Enter row, Shift+Space column) | Ctrl+Shift+D / I | derivative, integral |
| Ctrl+Shift+4 / 3 | sum, product | Ctrl+= , Ctrl+9, Ctrl+0 | ==, ≤, ≥ |
| `]` `{` `}` | program, local assignment, if | Ctrl+Shift+] , Ctrl+; | else, else if |
| Ctrl+Shift+' , Ctrl+] | for, while | Ctrl+\ | return |
| `"` or Ctrl+T | text | Ctrl+2 / Ctrl+7 | plot, solve block |
| Ctrl+6 | table (Tab next cell, Enter one row down) | Ctrl+. | symbolic (→) |
| Ctrl+Shift+A | collapsible area (around the selection) | Ctrl+Enter | page break |
| Tab, ↑/↓, Ctrl+Space | autocomplete | | |
| F5 | recalculate | F2 | functions |
| Ctrl+N / O / S | new tab, open, save | Ctrl+Z / Y | undo, redo |
| Ctrl+P, Ctrl+Shift+P, Ctrl+Alt+P | print, export to PDF, print preview | Ctrl+F, Ctrl+H | find, replace |
| Ctrl+Tab / Ctrl+Shift+Tab | next / previous tab | Ctrl+W | close tab |
| Ctrl+, | settings | Ctrl+Shift+F1 | welcome sheet |

Type a name (two letters or more) and a list of matching names appears: first those of your sheet, then functions
and units (`MP` → MPa). Tab accepts the suggestion, ↑/↓ chooses (Enter then accepts the choice), Esc closes the list;
Ctrl+Space also shows it after one letter. Functions get brackets with the cursor between them.

Prime keys for things MathPrime cannot do (yet) give a message in the status bar. The source (plain text) is under
View → Source (Ctrl+Shift+B). Saving writes each region's place on the sheet (`@column,row`). The menu keys are in
`keymap.txt` and the settings in `instellingen.txt` (Windows: `%APPDATA%\MathPrime`). You set them via File →
Settings… (Ctrl+,): language (on first start the language of Windows), theme (light, dark or like Windows),
background, blank sheet or welcome sheet at startup, zoom, significant digits in results, autocomplete and vsync (off
by default: the window then follows the mouse directly when you move it; on prevents tearing, after a restart).

MathPrime opens only one window: if you double-click a `.rkn` in Explorer while MathPrime is open, the file becomes a
tab in that window. Every minute MathPrime saves a copy of sheets with unsaved changes
(`%APPDATA%\MathPrime\herstel`); after a crash it asks at the next start whether to restore them. **Ctrl+F** searches
the sheet (Enter: next match), **Ctrl+H** replaces; with "Whole names" you rename a variable everywhere it is used, but
not in parts of other names or where the same letters are a unit (the `L` of litre above the definition of a variable
`L`).

**Ctrl+Alt+P** (File → Print preview) shows the sheet exactly as it will be in the PDF: pages, headers and footers,
and plots as fixed images. Esc goes back to editing. **Ctrl+P** prints: MathPrime makes that PDF and hands it to the
Windows PDF reader, which shows the print window (if it cannot print directly, the PDF opens). Export to PDF is
**Ctrl+Shift+P**.

File → **New from template** gives a report, an exam (questions as collapsible areas), a simply supported beam or a
heat balance, with a header and footer. Dragging regions snaps to the grid; if a region lands on another, it moves
down until there is room, and what you dragged stays selected.

Several worksheets sit in tabs below the ribbon, as in Prime. Opening a file (or dragging it onto the window) opens a
new tab; if the current sheet is still empty, it goes there. A • on the tab means unsaved changes; on Ctrl+W and when
closing, MathPrime asks per tab whether to save. An empty, unnamed sheet closes without a question. MathPrime starts
with a blank sheet; the example is under Help → Welcome sheet.

## Formula library and rearranging

**Library** (on the ribbon under Math) opens a library on the right with 99 formulas: mechanics, strength of
materials, fluids, thermodynamics, electrical engineering, geometry and wind energy. Search by name or quantity. Click
the quantity you are looking for ("Solve for: σ M W"). MathPrime puts the rearranged formula on the sheet (`W := M/σ =`),
with an input line (value 1, with a unit) above it for each quantity the sheet does not know yet. With **+ Save own
formula** you keep your own formulas; they are in `formules.txt` in the settings folder.

**Rearrange ▾** (also on the ribbon) rearranges the formula you are in for another quantity. If it occurs once,
MathPrime rewrites the formula (+ ↔ −, · ↔ /, xⁿ ↔ ⁿ√, sin ↔ asin, ln ↔ exp). If it occurs more often, you get a solve
block (Given/Find) with an initial value. In the tests every library formula is rearranged for every quantity and
calculated, with units.

## Opening Mathcad files

File → Open can also open a Mathcad Prime file (`.mcdx`), or drag it onto the window. Formulas, text, headings, plots,
solve blocks and images (PNG and BMP) come along, each in its place. The calculation settings of the sheet too: a
different first index (`ORIGIN`) and the seed of the random numbers (`SEED`); another unit system or other solver
tolerances are reported. What does not come over is a grey note in that place and in a list. The `.mcdx` file itself
stays unchanged: saving makes a `.rkn`. On the command line:

```text
mathprime-cli import exam.mcdx exam.rkn
mathprime-cli run exam.mcdx          # also directly
```

Tested on 11 real sheets: all 194 results that Mathcad stored in those files (including those with a unit, in SI, and
the error messages) come out of MathPrime exactly the same (`cargo test -p rb-mcdx -- --nocapture` shows the overview).

### Saving as Mathcad

File → **Save as Mathcad (.mcdx)…** (or `mathprime-cli export sheet.rkn sheet.mcdx`) writes a file that Mathcad Prime
opens and calculates: formulas, text, headings, images, plots and solve blocks. What Mathcad does not know (program
blocks, sweeps, lookup tables, uncertainties) goes in as text, with a message. Your own `.rkn` stays the working file.
There and back (.mcdx → MathPrime → .mcdx) gives the same results for all 11 real sheets.

### Comparing with Mathcad

`mathprime-cli vergelijk sheet.mcdx` (or a folder) puts the results Mathcad stored in a `.mcdx` next to those of
MathPrime. `tools/mathcad-vergelijk.ps1` does the whole chain with your own Mathcad Prime, via the official Automation
API: MathPrime saves the test sheets from `tests/corpus` as `.mcdx`, Mathcad calculates and saves them, and `vergelijk`
makes a report.

## Symbolic calculation (→)

Type an expression and press **Ctrl+.** (or type `->`). A keyword can follow it; the buttons are on the ribbon under
Operators → Symbolic.

| line | result |
|---|---|
| `(x + 1)^2 → expand` | x² + 2x + 1 |
| `x^2 - 5*x + 6 → factor` | (x − 3)(x − 2) |
| `x^2 - 4 == 0 → solve, x` | [−2; 2] |
| `a*x^2 + b → substitute, x = 2` | 4a + b |
| `2*x + 3*x → simplify` | 5x |
| `deriv(x^3 + 2*x, x, x) →` | 3x² + 2 |
| `integral(t, t, 0, x) → simplify` | x²/2 |
| `integral(x * exp(x), x, 0, 1) →` | 1 (by parts; also sin, cos, tan, exp, sinh, cosh and powers of a·x + b) |
| `sin(x) / x → limit, x = 0` | 1 (l'Hôpital; also `x = ∞`) |
| `cos(x) → series, x = 0, 5` | x⁴/24 − x²/2 + 1 (Taylor up to power 4) |
| `sqrt(2) * x → float, 4` | 1.414 x |

Names with a known value are filled in and your own functions written out, as in Mathcad. A value with a unit goes in
as number times unit, and units are then ordinary symbols (`F * x → 10 kN · x` with `F := 10 kN`). If only numbers and
units remain, that becomes one quantity: `F * a → 20 kN · m`, `1 m + 50 cm → 1.5 m`, `k * u == 10 N → solve, u` gives
`5 mm`. The work is done by our own code (`rb-core/src/symbolic.rs`).

## The worksheet format (`.rkn`)

Plain text, one region per line, so it compares and merges fine in git.

```text
# Heading
" Text line (starts with a quotation mark, as in Mathcad)
// comment

L := 2 m                   definition
q := 3 kN/m   // note
f(x, y) := x^2 + y         function

q * L =                    result, unit automatic
q * L = kN                 result in a chosen unit
q =                        shows 3 kN/m: the unit of the definition carries over
M.max := q * L^2 / 8       names can have a literal subscript (M.max) and unicode (σ, Δ)
M := q * L^2 / 8 = kN*m    define and show at once, as in Mathcad
I := 230 V / 50 Ω =        side by side binds tighter than * and /: (230 V)/(50 Ω);
                           so write ½ m v² as 1/2 * m * v^2
P == F * v                 a formula that is only shown (explanation)
s := concat("Hello ", "world")   text as a value (also strlen, substr, search, num2str, str2num)
M := READCSV("data.csv")   read a file (also READEXCEL("test.xlsx", "Sheet1!A2:C20"), READFILE)
WRITEEXCEL("out.xlsx", M) =  write (also WRITECSV, WRITEFILE); paths are relative to the sheet
image 200x120 data:image/png;base64,...   an image (in the file itself)
tabel x [mm], F [kN] {     table (Ctrl+6): each column becomes a vector with a unit,
    0, 0                   here x := [0; 10] * (mm) and F := [0; 2.5] * (kN)
    10, 2.5
}
area Input                 a collapsible area up to `area end`; collapsed: `area ingeklapt Input`
area end                   (the content still calculates but is hidden; click ▾ to collapse)
pagebreak                  what is below it starts on a new page
header Report {bestand}    header and footer on every page, with {pagina}, {paginas}, {bestand}, {datum}
footer page {pagina} of {paginas}
(x + 1)^2 → expand         symbolic (Ctrl+. or ->): x^2 + 2 * x + 1

sweep hh = 80 mm ; 160 mm ; 5 : 6 * M.max / (b * hh^2) = MPa    parameter sweep, 5 points
plot x = 0 m ; L : w(x), v(x) = mm    plot of functions (200 points), y-axis in mm
plot t = 0 s ; 2 s ; 500 : u(t)       with a chosen number of points
plot x.m : F.m                        measured points (two vectors)
plot f = 10 Hz ; 100 kHz : abs(H(f)) | log xy    logarithmic axes (Bode plot)
plot x = 0 ; 10 : sin(x) | y = -1 ; 1, x = 2 ; 8  fixed axis limits (without a unit: in the axis unit)
plot θ = 0 ; 2*π : 1 + cos(θ) | polar             polar plot (x and y equally large)
plot t = 0 s ; 10 s : F(t) ; u(t) = kN ; mm        after ';' the right y-axis, with its own unit
plot x = -3 ; 3, y = -3 ; 3 : f(x, y)             3D surface (drag rotates; 40 × 40 points)
plot x = -3 ; 3, y = -3 ; 3 : f(x, y) | contour   contour lines
plot t = 0 ; 20 : cos(t), sin(t), t | 3d          curve in space (drag rotates)
plot x = -2 ; 2, y = -2 ; 2 : -y, x | field       vector field (15 × 15 arrows)
@40,3 b := 2 m             region at column 40, row 3 (order: row, then column)
```

### Calculating

```text
M(x) := if(x <= a, R.A * x, R.A * x - F * (x - a))     piecewise; also if(v1, w1, v2, w2, ..., otherwise)
L > 3 m and F < 20 kN =                               comparisons and logic give 1 or 0
0 < x ≤ 1 =                                           chain of comparisons; also ==, !=, ≠, not, or

x := 0 m, 1 m .. 4 m                                  range with a step (i := 1..10: step 1)
M(x) = kN*m                                           table: one row per value of x
v := x^2                                              definition over a range: a column vector
h[i] := (80 + 40 * i) mm                              element by element, as in Mathcad

K := [300, -100; -100, 100] N/mm                      matrix: ',' between elements, ';' between rows
u := lsolve(K, [0; 1.5] kN)                           the units of the result follow by themselves
K^-1 = mm/N                                           also det(K), transpose(K), K[0, 1]; ORIGIN := 1 for 1-based

sum(1/k^2, k, 1, 100) =                               also Σ, prod and ∏; sum(v) adds up a vector
integral(M(x), x, 0 m, L) = kN*m^2                    also ∫ and limits ±∞
deriv(M(x), x, 2 m) = kN                              deriv(f, x, point, order) up to order 5

Z := R + 1i * ω * L.s + 1 / (1i * ω * C)              complex: 2i or 2j; Re, Im, arg, conj, abs
T := 20 °C                                            temperature with a zero point (also °F, degC, degF)
T + 5 Δ°C = °F                                        Δ°C and Δ°F for differences
```

### Tolerances and error propagation

```text
L := 2 m ± 1 mm                 a value with an uncertainty (± on the ribbon, under Operators)
b := 300 mm ± 2 mm
A := L * b
A =                             (0.6000 ± 0.0040) m^2: independent sources add in quadrature
L - L =                         0 m: the same source cancels; L + L has twice the uncertainty
sqrt(A) =                       functions work too (first order, as in the GUM)
nomval(A) =  unc(A) =           the value and the uncertainty separately
```

Solvers and plots use the nominal value; comparisons too. A value with ± cannot be an element of a matrix yet.

### Lookup tables

```text
IPE := tabel("IPE")             also "HEA", "HEB", "bout" (bolts), "boutklasse" (8.8), "materiaal", "beton"
IPE("IPE200").Iy =              1943.17 cm^4; also IPE("IPE 200", "Iy") and IPE(200).Iy
p := IPE("IPE300")              a row; p("A") = 53.8 cm^2
σ := M / IPE("IPE200").Wy
tabel("beton", "C30/37", "Ecm") =     directly, without a variable
```

Profiles have the columns `h b tw tf r A G Iy Wy Wply iy Iz Wz Wplz iz`. A, I, W and i are calculated from the geometry,
including the fillets, and tested against published tables. Bolts: `d P As A s`. Bolt classes: `fub fyb`. Materials:
`fy fu E rho` (S235 to S460, stainless steel, aluminium, titanium; steel for thickness up to 16 mm, the others typical
values). Concrete: `fck fckcube fcm fctm Ecm` (EN 1992-1-1). Check the values against your standard for a real design.

### Programming and solving

```text
steps(n) := {                   program block: indent as in Python, close with }
    k := 0                      an assignment in a block is local (also k ← 0)
    while n != 1                also: for k in 1..n, for k ∈ v, if / else if / else,
        if mod(n, 2) == 0            break, continue, return
            n := n / 2
        else
            n := 3 * n + 1
        k := k + 1
    k                           without return: the value of the last line
}

root(tan(z) - z, z, 4, 4.6) =   root between limits; root(f(x), x) starts at the current x
polyroots([-6, 11, -6, 1]) =    roots of -6 + 11x - 6x² + x³ (coefficients ascending)

F.1 := 1 kN                     initial values above the block
F.2 := 1 kN
Given                           solve block
F.1 + F.2 == 10 kN              constraints: ==, <, >, ≤, ≥
F.1 * 2 m == F.2 * 3 m
Find(F.1, F.2) = kN             also Minerr(x, ...), Minimize(f, x, ...), Maximize(f, x, ...)

Given
u''(t) + 2 ζ ω u'(t) + ω^2 u(t) == 0
u(0 s) == 10 mm
u'(0 s) == 0 m/s
u := odesolve(t, 2 s)           a function: u(1 s) = mm
                                system: x'(t) == y(t), y'(t) == -x(t), ... and then
                                [x; y] := odesolve([x; y], t, 10)
                                boundary values: w''''(x) == q/EI, w(0 m) == 0 m, w''(0 m) == 0,
                                w(L) == 0 m, w''(L) == 0, w := odesolve(x, L)   (shooting method)
rkfixed([1; 0], 0, π, 100, D) = matrix with t and y; D(t, y) := [y[1]; -y[0]]  (also Rkadapt, Bulstoer, and Radau/Stiffb/Stiffr for stiff equations)
sbval([0.5], 0, π/2, D, load, score) =   missing initial values (boundary value problem without Given); also bvalfit
Jacob(F, [1; 2]) =              Jacobian of a vector function

λ := eigenvals(M^-1 * K)        also eigenvecs, svd, svds, qr, lu, cholesky, rank, cond2
slope(x.m, F.m) = N/mm          also intercept, line, stderr, polyfit, linfit(vx, vy, F), genfit
interp(regress(vx, vy, 2), vx, vy, x) =   polynomial; also interp(loess(vx, vy, 0.5), vx, vy, x)
ksmooth(vx, vy, b)  medsmooth(vy, 5)  supsmooth(vx, vy)   smoothing; localmax(vy), localmin(vy) give indices; filterNaN(M)
interp(cspline(vx, vy), vx, vy, x) =    splines; linterp(vx, vy, x) linear
qnorm(0.975, 10 mm, 2 mm) =     distributions d/p/q: norm, lnorm, t, chisq, F, exp, unif, weibull
fft(v) =                        also ifft; any length
Jn(0.5, 2) =  Ai(1) =  js(1, x) =    Bessel also of non-integer order (Jn Yn In Kn), Airy Ai/Bi, spherical js/ys
ellipK(0.5) =                   elliptic integrals with the parameter m = k²: ellipK(m), ellipE(m), ellipE(φ, m), ellipF(φ, m), ellipPi(n, m), ellipPi(n, φ, m)
pmt(0.05/12, 360, 200000) =     financial: pv fv pmt nper rate npv irr mirr cnper crate eff nom cumint cumprinc fvadj fvc
```

* From top to bottom (and on a row from left to right): a name must be defined before the place where it is used.
* `5 kN`, `2 pi r`, `2(x+1)`: implicit multiplication. It has the same level as `*` and `/` (left-associative), so
  `1/2 x` is (1/2)·x. Use brackets for a denominator: `1/(2 x)`.
* `2i` is an imaginary number. With a range variable `i` write `40 * i` or `40 i`, not `40i`.
* A variable goes before a unit or built-in function with the same name (`m := 5` makes `m` a number).
* Angles are dimensionless: `sin(30 deg)` works, and `rpm` is `rev/min = 2π/60 rad/s`.
* `g` is the acceleration of gravity and `gm` a gram, as in Mathcad.

Units: SI base and derived units with prefixes (`kN`, `MPa`, `µm`, `kWh`, ...), plus `in`, `ft`, `lb`, `psi`, `bar`,
`hp`, `PS`, `L`, `rpm`, `deg`, ... and temperatures `°C`, `°F`, `Δ°C`, `Δ°F`, `°R`. The full table is in
`crates/rb-units/src/table.rs`.

Built-in functions (`:functies` in the REPL shows them all):

| group | functions |
|---|---|
| roots, powers, log | `sqrt cbrt nthroot exp ln log log2` |
| trigonometry | `sin cos tan cot sec csc asin acos atan acot atan2 sinh cosh tanh coth asinh acosh atanh acoth` |
| rounding and other | `abs sign floor ceil trunc round mod hypot min max Φ δ gcd lcm factorial (n!) combin permut` |
| complex | `Re Im arg conj` |
| special functions | `erf erfc Γ lnGamma Beta Ψ J0 J1 Jn Y0 Y1 Yn I0 I1 In K0 K1 Kn js ys Ai Bi ellipK ellipE ellipF ellipPi` |
| statistics | `sum prod mean median var Var stdev Stdev corr cvar percentile hist` |
| distributions | `dnorm pnorm qnorm` (and lnorm, t, chisq, F, exp, unif, weibull), `dbinom pbinom dpois ppois rnd runif rnorm` |
| vectors and matrices | `rows cols length last transpose det inverse identity augment stack submatrix diag tr lsolve dot cross norm sort csort rsort reverse match lookup vlookup` |
| linear algebra | `eigenvals eigenvecs eigenvec cholesky svd svds qr lu rank cond2 norm1 norm2 normi norme polyroots fft ifft Jacob` |
| interpolation and regression | `linterp lspline pspline cspline interp slope intercept line stderr polyfit linfit genfit regress loess ksmooth medsmooth supsmooth localmax localmin filterNaN` |
| solvers and ODEs | `root Find Minerr Minimize Maximize rkfixed Rkadapt Bulstoer Radau Stiffb Stiffr sbval bvalfit odesolve` |
| financial | `pv fv pmt nper rate npv irr mirr cnper crate eff nom cumint cumprinc fvadj fvc` |
| uncertainty and tables | `nomval unc tabel` |
| special forms | `if sum/Σ prod/∏ integral/∫ deriv` |

Functions of one number also work element by element on a vector or matrix (`sqrt([4, 9])`).

## Layout

| crate | task |
|---|---|
| `rb-units` | dimensions, units, quantities, temperature scales, display (`12.5 MPa`, `1.5 kN·m`), message translation |
| `rb-core` | lexer, parser (Pratt), values (complex, matrix, range, uncertain, table), evaluator, built-in functions, linear algebra, special functions and distributions, regression, solvers, ODEs, FFT |
| `rb-sheet` | worksheet format, positions, program blocks, solve blocks, plot regions, dependency graph, recalculation, sweeps |
| `rb-layout` | formula display: fractions, powers, roots, matrices, Σ/∏/∫ as boxes with text and lines |
| `rb-export` | display per region, page layout, drawing plots, SVG and PDF (fonts embedded) |
| `rb-cli` | binary `mathprime-cli` (run, deps, repl, export) |
| `rb-mcdx` | reading and writing Mathcad Prime files (own ZIP and XML code) |
| `rb-edit` | the formula editor: editable formula, behaviour of the Prime keys, layout with a cursor |
| `rb-gui` | binary `mathprime` (egui): worksheet canvas, ribbon, interactive plots, files |
| `rb-setup` | binary `mathprime-setup` (egui): the installer; the program files are appended as a ZIP |

The code and its comments are in Dutch; see [`CONTRIBUTING.md`](CONTRIBUTING.md).

## What MathPrime already does better than the original

* A text format that fits in git.
* Error messages that say what is wrong ("units do not match in addition: m and s"), with a caret at typing mistakes,
  and follow-up errors that point to the cause instead of everything turning red.
* Recalculation via a dependency graph: after a change, only the lines that depend on it are recalculated (tested, see
  `crates/rb-sheet/tests/sheet.rs`).
* `deps`: see where a value comes from and what it affects; unused definitions are marked.
* A warning if a name counts as a unit at the top but becomes a variable further down.
* Sweeps and results over a range with an error message per row instead of a failed calculation.
* A value appears in the unit in which you defined it (`3 kN/m`, `1450 rpm`, `20 °C`).
* `0.1 + 0.2 == 0.3` is true.
* `det`, `inverse` and `lsolve` derive the units of the result themselves.
* Solvers keep units: `Find` gives mm if the unknown started in mm, `odesolve` gives a function in the unit of the
  initial value, eigenvalues of M⁻¹·K come in 1/s².
* `Find` names the residual if there is no solution.
* Error propagation built in, with correlation handled correctly.

## Licences

MathPrime itself is under the MIT licence (`LICENSE`): you may use, change and distribute it, with the copyright line.
To help out: see `CONTRIBUTING.md`.

The icons in the interface are [Tabler Icons](https://tabler.io/icons) (outline, version 3.31.0), MIT licence,
© Paweł Kuna; the font is in `crates/rb-gui/assets/tabler-icons-outline.ttf` with the licence next to it
(`tabler-icons-LICENSE.txt`).

Mathcad and Mathcad Prime are trademarks of PTC Inc. MathPrime is an independent project and not affiliated with PTC.
