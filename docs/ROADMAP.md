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

Started so far: the theme engine with built-in and own themes, the title bar in the theme, importing and exporting
formula libraries, and a frame time display (Ctrl+Shift+F12). Still to do is marked below.

### Themes

Inspired by the theme system of [Odysseus](https://github.com/odysseus-dev/odysseus): a small palette from which
everything else is derived, presets, and your own themes as files.

- A theme is five colours: background, text, panel, border, accent. Sheet colours and shades of the ribbon are derived.
- The interface and the sheet follow the theme; the exported PDF stays black on white. To do: the remaining fixed colours (grid, plot lines, error red).
- Built-in themes named after the arcana.
- Own themes: pick colours in Settings with a live preview; export and import as a small text file.
- To do: subtle animated backgrounds, as an option.

### Title bar in the theme

Replace the standard Windows title bar with our own, in the theme colours: icon, file name, minimise, maximise and
close buttons, dragging (double-click maximises), and resizing from the edges. Keep Windows snapping working as far as
the window system allows. To do: test dragging, resizing and maximising by hand; this could not be automated.

### Modular library

One package format for formulas and lookup tables (plain text so it fits in git; today the same format as `formules.txt`, formulas only):

- import other libraries, export your own, extend them;
- to do: a manager window: which libraries are on, where they came from;
- to do: the built-in formulas and tables become packages themselves;
- to do: tables from a package work the same as the built-in ones: `tabel("mypackage/IPE")`.

### Faster and smoother

Measured in 1.8: a sheet of 486 lines calculates in 0.25 s and a frame costs about 3 ms, so there is no single
bottleneck to fix blindly. The plan is therefore to measure first:

- a frame time overlay for development;
- to do: benchmarks with large sheets and many plots, in the test suite;
- to do: then the biggest costs, for example keeping the layout of unchanged regions and repainting less when idle.

## Later

- `CreateSpace`/`CreateMesh`, PCA (`Nipals`), Design of Experiments, image processing.
- An English command line.
- Parallel recalculation of independent regions; WebAssembly for use in the browser.
