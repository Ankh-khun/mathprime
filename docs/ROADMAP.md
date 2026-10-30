# MathPrime roadmap

Internal code name: **Justice** (major arcana XI). The scales fit a worksheet in which both sides of an equation, and
all the units, have to balance. The product stays called MathPrime.

## 1.9

Done in this release: English error messages, installer and repository; no save prompt for an empty sheet; `Jacob`,
elliptic integrals and `supsmooth`; 3D curves and vector fields; lookup tables; tolerances with error propagation.

Open:

- **Areas and headers/footers in `.mcdx`.** Needs a sample sheet made in Mathcad Prime with an area (Ctrl+Shift+A) and a
  header text: the format of these elements cannot be guessed safely. None of the available sheets contain them.

## 2.0

The work happens on the branch `2.0-dev`.

### Themes

Inspired by the theme system of [Odysseus](https://github.com/odysseus-dev/odysseus): a small palette from which
everything else is derived, presets, and your own themes as files.

- A theme is five colours: background, text, panel, border, accent. Sheet colours and shades of the ribbon are derived.
- All fixed colours in `rb-gui` go through the theme; the exported PDF stays black on white.
- Built-in themes named after the arcana.
- Own themes: pick colours in Settings with a live preview; export and import as a small text file.
- Later: subtle animated backgrounds, as an option.

### Title bar in the theme

Replace the standard Windows title bar with our own, in the theme colours: icon, file name, minimise, maximise and
close buttons, dragging (double-click maximises), and resizing from the edges. Keep Windows snapping working as far as
the window system allows.

### Modular library

One package format for formulas and lookup tables (`.mplib`, plain text so it fits in git):

- import other libraries, export your own, extend them;
- a manager window: which libraries are on, where they came from;
- the built-in formulas and tables become packages themselves;
- tables from a package work the same as the built-in ones: `tabel("mypackage/IPE")`.

### Faster and smoother

Measured in 1.8: a sheet of 486 lines calculates in 0.25 s and a frame costs about 3 ms, so there is no single
bottleneck to fix blindly. The plan is therefore to measure first:

- a frame time overlay for development;
- benchmarks with large sheets and many plots, in the test suite;
- then the biggest costs, for example keeping the layout of unchanged regions and repainting less when idle.

## Later

- `CreateSpace`/`CreateMesh`, PCA (`Nipals`), Design of Experiments, image processing.
- An English command line.
- Parallel recalculation of independent regions; WebAssembly for use in the browser.
