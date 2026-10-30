# Function inventory: Mathcad Prime versus MathPrime

Purpose: tick off what is still missing from Mathcad. Per category: examples, difficulty and since which version.

**Column "source"**: *PTC help* = the category and examples are in the PTC documentation; *own knowledge* = from
knowledge of Mathcad, still to be checked against an installation or the PTC help. This is a planning list, not a
complete list of functions: take final names and arguments from the official help.

Difficulty: **S** hours, **M** days, **L** weeks, **XL** months (relative).

## Present

| part | what | since |
|---|---|---|
| Operators | `+ - * / ^`, implicit multiplication, leading minus, `n!` | v0.1, `!` v0.2 |
| Roots and powers | `sqrt cbrt nthroot`, fractional powers if the unit allows it | v0.1, `nthroot` v0.3 |
| Trigonometry | `sin cos tan cot sec csc asin acos atan acot atan2` (angles dimensionless, `deg` and `rad`) | v0.1, rest v0.2 |
| Hyperbolic | `sinh cosh tanh coth asinh acosh atanh acoth` | v0.1, rest v0.2 |
| Log and exponential | `exp ln log log2` | v0.1 |
| Rounding | `floor ceil trunc round` | v0.1 |
| Other | `abs min max hypot mod sign` | v0.1, `sign` v0.2 |
| Units | SI + common non-SI, prefixes, dimension analysis, display in a chosen unit | v0.1 |
| Display unit from the definition | `E := 5 kJ` shows joule, `q := 3 kN/m` shows `kN/m` | v0.2 |
| Temperature scales | `°C`, `°F` (also `degC`, `degF`) with a zero point; `Δ°C`, `Δ°F`, `°R`; `= °C` | v0.2 |
| Comparison and logic | `< ≤ > ≥ == ≠` (also `<= >= !=`), chains `0 < x ≤ 1`, `and or not` (also `∧ ∨ ¬`) | v0.2 |
| Piecewise | `if(v, a, b)`, `if(v1, a1, v2, a2, ..., otherwise)`, `Φ` (Heaviside), `δ` (Kronecker) | v0.2 |
| Ranges | `i := 1..10`, `x := 0, 0.5 .. 3`, implicit iteration, table per value | v0.2 |
| Program blocks | `f(n) := {` ... `}` with local assignment (`:=`, `←`), `if`/`else if`/`else`, `for k in 1..n`, `for k ∈ v`, `while`, `break`, `continue`, `return` | v0.3 |
| Vector and matrix | `[1, 2; 3, 4]`, `v[i]`, `M[i, j]`, `v[i] := ...`, `ORIGIN`, `+ - *`, `M^n`, `M^-1`, complex elements | v0.2, complex v0.3 |
| Matrix functions | `rows cols length last transpose det inverse identity augment stack submatrix diag tr lsolve dot cross norm sort reverse` | v0.2 |
| Linear algebra | `eigenvals eigenvecs eigenvec cholesky svd svds qr lu rank cond2 norm1 norm2 normi norme`, `Jacob` | v0.3, v1.9 |
| Search and sort | `csort rsort match lookup vlookup` | v0.3 |
| Sum and product | `sum(f, i, a, b)` / `Σ`, `prod(f, i, a, b)` / `∏`, `sum(v)`, `prod(v)` | v0.2 |
| Calculus (numeric) | `integral(f, x, a, b)` / `∫` (also with ±∞), `deriv(f, x, point[, order])` | v0.2 |
| Complex numbers | `2i`, `3j`, `Re Im arg conj abs`, complex `sqrt ln exp ^` | v0.2 |
| Number theory and combinatorics | `gcd lcm factorial combin permut` | v0.2 |
| Statistics | `mean median var Var stdev Stdev corr cvar percentile hist` | v0.2, rest v0.3 |
| Distributions | `d`/`p`/`q` + `norm lnorm t chisq F exp unif weibull`; `dbinom pbinom dpois ppois`; `rnd runif rnorm` | v0.3 |
| Special functions | `erf erfc Γ (gamma) lnGamma Beta Ψ (digamma)`, Bessel `J0 J1 Jn Y0 Y1 Yn I0 I1 In K0 K1 Kn` (`Jn Yn In Kn` also of non-integer order), spherical `js ys`, Airy `Ai Bi`, elliptic `ellipK ellipE ellipF ellipPi` | v0.3, v1.8, v1.9 |
| Interpolation | `linterp`, `lspline`/`pspline`/`cspline` + `interp` | v0.3 |
| Regression | `slope intercept line stderr polyfit linfit genfit`; `regress` and `loess` (with `interp`), `ksmooth`, `medsmooth`, `supsmooth`, `localmax`, `localmin`, `filterNaN` | v0.3, v1.7, v1.9 |
| Solving | `root(f, x)`, `root(f, x, a, b)`, `polyroots`, `lsolve`, `Given` with `Find`/`Minerr`/`Minimize`/`Maximize` | v0.3 |
| Differential equations | `rkfixed`, `Rkadapt`, `Bulstoer`, `Radau`, `Stiffb`, `Stiffr`, `odesolve` (any order; systems `[x; y] := odesolve([x; y], t, b)`; initial and boundary value problems), `sbval`, `bvalfit` | v0.3, v1.6, v1.7, v1.8 |
| Signal processing | `fft ifft cfft icfft` (any length; numpy normalisation) | v0.3 |
| Worksheet | definitions, functions, results, text, headings, sweeps, positions `@column,row`, dependency graph, multi-line blocks, solve blocks | v0.1, positions v0.2, blocks v0.3 |
| Plots | `plot x = a ; b [; n] : f(x), g(x) [= unit]`, `plot vx : vy` (measured points), legend, units on the axes, interactive in the interface; logarithmic axes and fixed limits (`\| log xy, y = a ; b`); polar plots (`\| polar`), second y-axis (`left ; right`), 3D surfaces and contour lines (`plot x = .., y = .. : f(x, y) [\| contour]`), 3D curves (`\| 3d`), vector fields (`\| field`) | v0.4, v1.4, v1.7, v1.9 |
| Formula display | fractions, powers, indices, roots, brackets, matrices, Σ/∏/∫, d/dx, program blocks, units in results | v0.4 |
| Export | PDF (A4, embedded fonts) and SVG, also via `mathprime-cli export` | v0.4 |
| Text | strings as values; `concat strlen substr search num2str str2num str2vec vec2str IsString error` | v0.8 |
| Files | `READCSV READEXCEL READFILE WRITECSV WRITEEXCEL WRITEFILE` (.csv, .prn, .txt, .xlsx) | v0.9 |
| Symbolic | `→` with `simplify expand factor solve substitute limit series float`; `deriv`, integrals (polynomials, standard functions of a·x + b, by parts); units take part (no `assume`) | v0.9, v1.6, v1.7 |
| Financial | `pv fv pmt nper rate npv irr mirr cnper crate eff nom cumint cumprinc fvadj fvc` (sign convention of Excel) | v1.8 |
| Uncertainty (own addition) | `2 m ± 1 mm`, first order propagation per source, `nomval unc` | v1.9 |
| Lookup tables (own addition) | `tabel("IPE")`, also HEA, HEB, bolts, bolt classes, materials, concrete | v1.9 |

## Still to build

| category | examples | difficulty | source |
|---|---|---|---|
| Differential equations (rest) | `stiffb`/`stiffr`/`rkadapt` (end point only) | S | own knowledge |
| Data analysis (rest) | PCA (`Nipals`) | L | PTC help |
| Design of Experiments | design matrices, factor screening, ANOVA, regression, Monte Carlo | L | PTC help |
| Plots (rest) | `CreateSpace`/`CreateMesh` | M | PTC help (Graphing) |
| Image processing | colour, binarisation, convolution, morphology, transforms | L | PTC help; probably low priority |

## Interface and operation

| part | difficulty | note |
|---|---|---|
| 2D formula display (fractions, roots, indices) | L | **done in v0.4** (`rb-layout`) |
| Graphical interface | L | **done in v0.4** (`mathprime`); **v0.5**: typing on the sheet as in Prime, dragging, selecting, undo, ribbon with tabs, background with dots/squares/lines |
| Structure-based formula editor | XL | cursor behaviour, growing the selection, inserting around the selection |
| Keyboard shortcuts | M | **v0.5**: the keys of Prime 10 on the sheet (`rb-edit/src/keys.rs`); `keymap.txt` for the menu actions |
| PDF export | M | **done in v0.4** (and SVG) |
| `.mcdx` import | L | **v0.6**; best effort, never complete |
| Formula library and rearranging for the unknown | M | **v0.7** (own addition; Mathcad has symbolic `solve`) |
| Text as a value (strings) | M | **v0.8** |
| Saving as Mathcad (.mcdx) | L | **v0.8** |
| Check against Mathcad itself | M | **v0.8** (`tools/mathcad-vergelijk.ps1` + `mathprime-cli vergelijk`) |
| Autocomplete | M | **v1.0** (names of the sheet, functions and units; Tab, ↑↓, Ctrl+Space) |
| Tables (Ctrl+6) | M | **v1.0** |
| Collapsible areas, page breaks, headers and footers | M | **v1.0**; page breaks also in .mcdx; areas and headers/footers in .mcdx still open |
| Installer and updates | M | **v1.8** |
| Language | M | **v1.7** interface, **v1.8** library, **v1.9** error messages and installer |
