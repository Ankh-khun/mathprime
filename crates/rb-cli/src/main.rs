use rb_core::{builtins, prefix_lines, with_stack, Env, Error};
use rb_sheet::Sheet;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;

const HELP: &str = "\
MathPrime — een rekenblad met eenheden

Gebruik:
  mathprime-cli run <bestand.rkn> [--watch] [--strikt]
                                       rekent een werkblad door en toont het resultaat
                                       (--watch herberekent bij elke opslag)
                                       (--strikt toont ook waarschuwingen als 'L is ook een eenheid')
  mathprime-cli deps <bestand.rkn> [naam]  toont afhankelijkheden (van één naam, of een overzicht)
  mathprime-cli export <bestand.rkn> <uit.pdf|uit.svg> [--watch] [--strikt]
                                       opgemaakt werkblad (formules, tabellen, grafieken) als PDF of SVG
                                       (--watch exporteert opnieuw bij elke opslag)
  mathprime-cli import <bestand.mcdx> <uit.rkn>
                                       Mathcad Prime-bestand omzetten (run en export lezen .mcdx ook direct)
  mathprime-cli vergelijk <bestand.mcdx|map>... [--rapport uit.md]
                                       uitkomsten van Mathcad (in het .mcdx) naast die van MathPrime
  mathprime-cli repl                   interactieve rekenmachine
  mathprime-cli help                   deze tekst

Syntax (zie ook README.md):
  a := 5 m               definitie            f(x) := x^2     functie
  a * 2 =                resultaat            a * 2 = cm      resultaat in gekozen eenheid
  i := 1..10             bereik               x := 0, 0.5 .. 3   bereik met stap
  v[i] := i^2            element van vector   M[i, j] := i + j   element van matrix
  [1, 2; 3, 4]           matrix               v[0]            index (ORIGIN := 1 voor 1-gebaseerd)
  if(x < 0, -x, x)       voorwaarde           a <= b and b != c   vergelijken en logica (1 of 0)
  sum(i^2, i, 1, 10)     som (ook prod)       integral(x^2, x, 0, 1)   integraal
  deriv(x^3, x, 2)       afgeleide            3 + 4i          complex getal
  20 °C                  temperatuur          T = °C          tonen in °C (ook °F, Δ°C)
  f(n) := {              programmablok over meerdere regels (inspringen; for, while, if/else,
      s := 0             break, continue, return), afgesloten met een regel }
      for k in 1..n
          s := s + k
      s
  }
  root(x^2 - 2, x, 0, 2) nulpunt              root(f(x), x)   vanaf de huidige waarde van x
  Given                  solve block: daaronder vergelijkingen (x + y == 10, x > 0), afgesloten
                         met Find(x, y), Minerr(x, y), Minimize(f, x, y) of Maximize(f, x, y)
  y := odesolve(t, 10)   in een Given-blok met y''(t) + y(t) == 0, y(0) == 1, y'(0) == 0
  eigenvals(M)  lsolve(A, b)  polyroots(v)  linfit(vx, vy, F)  rkfixed(y, 0, 1, 100, D)  fft(v)
  \" tekst               tekstregel           # kop           kop
  sweep x = 1 m ; 5 m ; 5 : x^2 =             parametersweep (5 punten van 1 m tot 5 m)
  @40,3 b := 2 m         regio op kolom 40, rij 3 (volgorde: rij, dan kolom)
";

const REPL_HELP: &str = "\
Typ een definitie (a := 5 m), een uitdrukking (a * 2) of een resultaatregel (a * 2 = cm).
Commando's:
  :vars        toon alle variabelen
  :functies    toon eigen en ingebouwde functies
  :help        deze tekst
  :quit        stoppen (ook Ctrl-D)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => match args.get(1) {
            Some(path) => {
                let strict = args.iter().any(|a| a == "--strikt");
                let watch = args.iter().any(|a| a == "--watch");
                if watch {
                    watch_file(path, || run(path, strict))
                } else {
                    run(path, strict)
                }
            }
            None => usage_error("run: geef een bestand op"),
        },
        Some("export") => match (args.get(1), args.get(2)) {
            (Some(src), Some(dst)) => {
                let strict = args.iter().any(|a| a == "--strikt");
                let watch = args.iter().any(|a| a == "--watch");
                if watch {
                    watch_file(src, || export(src, dst, strict))
                } else {
                    export(src, dst, strict)
                }
            }
            _ => usage_error("export: geef een werkblad en een uitvoerbestand (.pdf of .svg) op"),
        },
        Some("import") => match (args.get(1), args.get(2)) {
            (Some(src), Some(dst)) => import(src, dst),
            _ => usage_error("import: geef een .mcdx-bestand en een uitvoerbestand (.rkn) op"),
        },
        Some("vergelijk") => vergelijk(&args[1..]),
        Some("deps") => match args.get(1) {
            Some(path) => deps(path, args.get(2).map(String::as_str)),
            None => usage_error("deps: geef een bestand op"),
        },
        // eigen functies mogen diep recursief zijn: de REPL draait op een thread met een ruime stack
        Some("repl") => with_stack(repl),
        Some("help") | Some("--help") | Some("-h") | None => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some(other) => usage_error(&format!("onbekend commando '{other}'")),
    }
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("{msg}\n");
    eprint!("{HELP}");
    ExitCode::from(2)
}

fn watch_file(path: &str, mut task: impl FnMut() -> ExitCode) -> ExitCode {
    let mut last_mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let _ = task();
    println!("\n[mathprime-cli] wacht op wijzigingen in '{path}' (--watch, druk op Ctrl+C om te stoppen)...");

    loop {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if mtime.is_some() && mtime != last_mtime {
            last_mtime = mtime;
            if io::stdout().is_terminal() {
                print!("\x1B[2J\x1B[1;1H");
            } else {
                println!("\n--- gewijzigd: {path} ---");
            }
            let _ = io::stdout().flush();
            let _ = task();
            println!("\n[mathprime-cli] wacht op wijzigingen in '{path}' (--watch, druk op Ctrl+C om te stoppen)...");
        }
    }
}

/// Een Mathcad Prime-bestand als bron van MathPrime.
fn read_mcdx(path: &str) -> Result<rb_mcdx::Import, ExitCode> {
    let grid = mcdx_grid();
    std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| rb_mcdx::import(&b, grid)).map_err(|e| {
        eprintln!("kan '{path}' niet inlezen: {e}");
        ExitCode::from(2)
    })
}

fn import(src: &str, dst: &str) -> ExitCode {
    let imp = match read_mcdx(src) {
        Ok(i) => i,
        Err(code) => return code,
    };
    if let Err(e) = std::fs::write(dst, &imp.source) {
        eprintln!("kan '{dst}' niet schrijven: {e}");
        return ExitCode::from(2);
    }
    println!("geschreven: {dst} ({} regio's)", imp.regions);
    for n in &imp.notes {
        println!("  niet overgenomen: {n}");
    }
    ExitCode::SUCCESS
}

fn read_sheet(path: &str) -> Result<Sheet, ExitCode> {
    let named = |mut s: Sheet| {
        let name = std::path::Path::new(path).file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
        s.set_name(&name);
        s
    };
    if path.to_lowercase().ends_with(".mcdx") {
        return read_mcdx(path).map(|i| named(Sheet::load_in(&i.source, &dir_of(path))));
    }
    match std::fs::read_to_string(path) {
        Ok(src) => Ok(named(Sheet::load_in(&src, &dir_of(path)))),
        Err(e) => {
            eprintln!("kan '{path}' niet lezen: {e}");
            Err(ExitCode::from(2))
        }
    }
}

fn run(path: &str, strict: bool) -> ExitCode {
    let sheet = match read_sheet(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    print!("{}", sheet.render_with(strict));
    let errors = sheet.error_count();
    if errors > 0 {
        eprintln!("\n{errors} regel(s) met een fout");
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// De rasterafmetingen van het blad (voor het omrekenen van posities van en naar Mathcad).
fn mcdx_grid() -> rb_mcdx::Grid {
    let grid = rb_export::doc::Grid::new(&rb_export::Setup::a4());
    rb_mcdx::Grid { col_w: grid.col_w, row_h: rb_export::doc::ROW_H }
}

/// Opslaan als Mathcad Prime-bestand.
fn export_mcdx(src: &str, dst: &str) -> ExitCode {
    let source = if src.to_lowercase().ends_with(".mcdx") {
        match read_mcdx(src) {
            Ok(i) => i.source,
            Err(code) => return code,
        }
    } else {
        match std::fs::read_to_string(src) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("kan '{src}' niet lezen: {e}");
                return ExitCode::from(2);
            }
        }
    };
    let out = rb_mcdx::export(&source, mcdx_grid());
    if let Err(e) = std::fs::write(dst, &out.bytes) {
        eprintln!("kan '{dst}' niet schrijven: {e}");
        return ExitCode::from(2);
    }
    println!("geschreven: {dst}");
    for n in &out.notes {
        println!("  {n}");
    }
    ExitCode::SUCCESS
}

fn export(src: &str, dst: &str, strict: bool) -> ExitCode {
    if dst.to_lowercase().ends_with(".mcdx") {
        return export_mcdx(src, dst);
    }
    let sheet = match read_sheet(src) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let lower = dst.to_lowercase();
    let bytes = if lower.ends_with(".pdf") {
        let title = std::path::Path::new(src).file_stem().map_or("MathPrime".into(), |s| s.to_string_lossy().into_owned());
        rb_export::export_pdf(&sheet, &title, strict)
    } else if lower.ends_with(".svg") {
        rb_export::export_svg(&sheet, strict).into_bytes()
    } else {
        return usage_error("export: het uitvoerbestand moet eindigen op .pdf of .svg");
    };
    if let Err(e) = std::fs::write(dst, bytes) {
        eprintln!("kan '{dst}' niet schrijven: {e}");
        return ExitCode::from(2);
    }
    let errors = sheet.error_count();
    if errors > 0 {
        eprintln!("geschreven: {dst} ({errors} regel(s) met een fout)");
        ExitCode::from(1)
    } else {
        eprintln!("geschreven: {dst}");
        ExitCode::SUCCESS
    }
}

fn deps(path: &str, name: Option<&str>) -> ExitCode {
    let sheet = match read_sheet(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    match name {
        None => print!("{}", sheet.overview()),
        Some(n) => match sheet.explain(n) {
            Some(text) => print!("{text}"),
            None => {
                eprintln!("'{n}' wordt in dit blad nergens gedefinieerd");
                return ExitCode::from(1);
            }
        },
    }
    ExitCode::SUCCESS
}

fn print_error(line: &str, e: &Error) {
    match e.position() {
        Some(pos) => {
            let col = line.get(..pos).map_or(0, |s| s.chars().count());
            eprintln!("{line}\n{}^ {e}", " ".repeat(col));
        }
        None => eprintln!("✗ {e}"),
    }
}

fn repl() -> ExitCode {
    let interactive = io::stdin().is_terminal();
    if interactive {
        println!("MathPrime REPL — typ :help voor hulp, :quit om te stoppen");
    }
    let mut env = Env::new();
    let stdin = io::stdin();
    loop {
        if interactive {
            print!("rb> ");
            let _ = io::stdout().flush();
        }
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("leesfout: {e}");
                return ExitCode::from(2);
            }
        }
        let mut line = line.trim().trim_start_matches('\u{feff}').to_string();
        if line.is_empty() {
            continue;
        }
        // een programmablok: doorlezen tot een regel `}`
        if line.ends_with('{') && (line.contains(":=") || line.contains('←')) {
            loop {
                if interactive {
                    print!("..> ");
                    let _ = io::stdout().flush();
                }
                let mut more = String::new();
                match stdin.lock().read_line(&mut more) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let more = more.trim_end();
                line.push('\n');
                line.push_str(more);
                if more.trim() == "}" {
                    break;
                }
            }
        }
        let line = line.as_str();
        match line {
            ":quit" | ":q" | ":exit" => break,
            ":help" | ":h" => {
                println!("{REPL_HELP}");
                continue;
            }
            ":vars" => {
                for n in env.var_names() {
                    if let Some(v) = env.render_var(&n) {
                        println!("{}", prefix_lines(&format!("{n} = "), &v));
                    }
                }
                continue;
            }
            ":functies" => {
                println!("eigen:      {}", env.fn_names().join(", "));
                println!("ingebouwd:  {}", builtins::NAMES.join(", "));
                continue;
            }
            _ => {}
        }
        match env.run_line(line) {
            Ok(Some(text)) => println!("{text}"),
            Ok(None) => {}
            Err(e) => print_error(line, &e),
        }
    }
    ExitCode::SUCCESS
}

/// Vergelijk de uitkomsten die Mathcad in .mcdx-bestanden bewaarde met die van MathPrime.
fn vergelijk(args: &[String]) -> ExitCode {
    let mut files = Vec::new();
    let mut report_path = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--rapport" {
            report_path = it.next().cloned();
            continue;
        }
        let p = std::path::Path::new(a);
        if p.is_dir() {
            let mut found: Vec<_> = std::fs::read_dir(p)
                .map(|d| d.flatten().map(|e| e.path()).filter(|q| q.extension().is_some_and(|x| x.eq_ignore_ascii_case("mcdx"))).collect())
                .unwrap_or_default();
            found.sort();
            files.extend(found);
        } else {
            files.push(p.to_path_buf());
        }
    }
    if files.is_empty() {
        return usage_error("vergelijk: geef een of meer .mcdx-bestanden of een map");
    }
    let (mut same, mut differ, mut error, mut premium) = (0, 0, 0, 0);
    let mut md = String::from("# MathPrime naast Mathcad\n\n");
    for p in &files {
        let imp = match read_mcdx(&p.to_string_lossy()) {
            Ok(i) => i,
            Err(_) => continue,
        };
        let report = rb_mcdx::compare::compare(&imp);
        use rb_mcdx::compare::Status;
        let (s, d, e, pr) = (report.count(Status::Gelijk), report.count(Status::Anders), report.count(Status::Fout), report.count(Status::Premium));
        println!("{}: {s} gelijk, {d} anders, {e} met een fout in MathPrime, {pr} niet te vergelijken (premium in Mathcad)", p.display());
        for r in report.rows.iter().filter(|r| matches!(r.status, Status::Anders | Status::Fout)) {
            println!("    {}  →  MathPrime {}  |  Mathcad {}", r.region, r.mathprime, r.mathcad);
        }
        md.push_str(&report.markdown(&p.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()), false));
        md.push('\n');
        same += s;
        premium += pr;
        differ += d;
        error += e;
    }
    println!("\nTotaal: {same} gelijk, {differ} anders, {error} met een fout in MathPrime, {premium} niet te vergelijken (premium-functie in Mathcad)");
    if let Some(path) = report_path {
        if let Err(e) = std::fs::write(&path, md) {
            eprintln!("kan '{path}' niet schrijven: {e}");
            return ExitCode::from(2);
        }
        println!("rapport: {path}");
    }
    if differ + error > 0 { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

/// De map van een bestand (voor paden in READCSV, ...); de huidige map als er geen is.
fn dir_of(path: &str) -> std::path::PathBuf {
    match std::path::Path::new(path).parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    }
}
