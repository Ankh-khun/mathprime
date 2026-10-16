//! rb-core: lexer, parser en evaluator voor MathPrime.
//!
//! Syntaxis (tekstinvoer, zo dicht mogelijk bij Mathcad):
//!
//! ```text
//! a := 5 m                  definitie
//! f(x, y) := x^2 + y        functiedefinitie
//! i := 1..10                bereik (impliciete iteratie, zoals in Mathcad)
//! v[i] := i^2               element van een vector (of M[i, j] van een matrix)
//! a * 2 =                   resultaat tonen (automatische eenheid)
//! a * 2 = cm                resultaat tonen in een gekozen eenheid
//! ```

pub mod ast;
pub mod base64;
pub mod builtins;
pub mod solve;
pub mod strings;
pub mod symbolic;
mod error;
mod eval;
pub mod finance;
pub mod fit;
mod lexer;
pub mod linalg;
pub mod numeric;
mod parser;
mod render;
pub mod special;
pub mod value;

pub use ast::{BStmt, BinOp, BindOp, CmpOp, Expr, Show, Stmt, SymOp};
pub use error::Error;
pub use eval::{constant_value, Defined, Env, FileIo, Outcome, UserFn};
pub use lexer::{lex, Tok, Token};
pub use parser::{parse_expr_str, parse_stmt};
pub use render::{complex_text, format_complex, format_rows, matrix_cells, prefix_lines, render_value, scalar_parts, Display, MatrixCells};
pub use value::{Complex, Matrix, RangeVal, Value};

/// Stackgrootte voor [`with_stack`].
pub const EVAL_STACK: usize = 64 << 20;

/// Voer `f` uit op een thread met een ruime stack ([`EVAL_STACK`]). Eigen functies mogen tot 200
/// niveaus diep recursief zijn; dat past niet altijd in de standaardstack (1 MB voor de
/// hoofdthread op Windows, 2 MB voor andere threads), zeker niet in een debug-build.
pub fn with_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| {
        let handle = std::thread::Builder::new()
            .stack_size(EVAL_STACK)
            .spawn_scoped(s, f)
            .expect("kan geen rekenthread starten");
        handle.join().unwrap_or_else(|e| std::panic::resume_unwind(e))
    })
}
