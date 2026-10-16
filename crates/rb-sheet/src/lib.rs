//! rb-sheet: werkblad, afhankelijkheidsgraaf en herberekening voor MathPrime.
//!
//! Een werkblad is een lijst regio's (tekst, definities, resultaten, sweeps) die van boven naar
//! beneden wordt doorgerekend. De afhankelijkheidsgraaf zorgt dat bij een wijziging alleen de
//! regio's worden herberekend die er echt van afhangen.

mod graph;
pub mod io;
mod parse;
pub mod plot;
mod sheet;
pub mod table;

pub use graph::{Graph, Issue, Warning};
pub use parse::{mark_given_blocks, parse_kind, parse_sheet, sort_regions, Image, Kind, Pos, Region, SweepSpec, SOLVERS};
pub use plot::{PlotData, PlotOptions, PlotSpec, PlotX, Series, Surface};
pub use sheet::{plot_summary, Output, Sheet};
pub use table::TableSpec;
