//! Functies op tekst (strings), met de namen en het gedrag van Mathcad. Posities in een tekst
//! beginnen bij 0, net als in Mathcad (dat ORIGIN standaard niet voor tekst gebruikt).

use crate::error::Error;
use crate::value::{Matrix, Value};
use rb_units::{format_number, Quantity};

/// Namen van de tekstfuncties (voor de functielijst).
pub const NAMES: &[&str] = &["concat", "strlen", "substr", "search", "str2num", "num2str", "str2vec", "vec2str", "IsString", "error"];

fn err(name: &str, msg: &str) -> Error {
    Error::Domain(format!("{name}: {msg}"))
}

fn text<'a>(name: &str, v: &'a Value, what: &str) -> Result<&'a str, Error> {
    match v {
        Value::Str(s) => Ok(s),
        other => Err(err(name, &format!("{what} moet tekst zijn, geen {}", other.kind()))),
    }
}

/// Een geheel getal ≥ 0 (een positie of lengte).
fn count(name: &str, v: &Value, what: &str) -> Result<usize, Error> {
    let x = v.as_quantity(what)?.dimensionless_value(what)?;
    if x.fract() != 0.0 || x < 0.0 {
        return Err(err(name, &format!("{what} moet een geheel getal ≥ 0 zijn")));
    }
    Ok(x as usize)
}

fn num(x: f64) -> Value {
    Value::Num(Quantity::scalar(x))
}

/// Roep een tekstfunctie aan; `None` als `name` er geen is.
pub fn call(name: &str, a: &[Value]) -> Option<Result<Value, Error>> {
    if !NAMES.contains(&name) {
        return None;
    }
    Some(call_inner(name, a))
}

fn call_inner(name: &str, a: &[Value]) -> Result<Value, Error> {
    let arity = |n: usize| {
        if a.len() == n {
            Ok(())
        } else {
            Err(err(name, &format!("verwacht {n} argument(en), kreeg er {}", a.len())))
        }
    };
    match name {
        "concat" => {
            let mut s = String::new();
            for v in a {
                s.push_str(text(name, v, "elk argument")?);
            }
            Ok(Value::Str(s))
        }
        "strlen" => {
            arity(1)?;
            Ok(num(text(name, &a[0], "het argument")?.chars().count() as f64))
        }
        "substr" => {
            arity(3)?;
            let s: Vec<char> = text(name, &a[0], "het eerste argument")?.chars().collect();
            let start = count(name, &a[1], "de beginpositie")?.min(s.len());
            let n = count(name, &a[2], "de lengte")?;
            Ok(Value::Str(s[start..(start + n).min(s.len())].iter().collect()))
        }
        "search" => {
            arity(3)?;
            let s: Vec<char> = text(name, &a[0], "het eerste argument")?.chars().collect();
            let sub: Vec<char> = text(name, &a[1], "het tweede argument")?.chars().collect();
            let from = count(name, &a[2], "de beginpositie")?;
            let found = (from..=s.len().saturating_sub(sub.len())).find(|&k| s.get(k..k + sub.len()) == Some(&sub[..]));
            Ok(num(found.map_or(-1.0, |k| k as f64)))
        }
        "str2num" => {
            arity(1)?;
            let s = text(name, &a[0], "het argument")?.trim();
            s.parse::<f64>().map(num).map_err(|_| err(name, &format!("'{s}' is geen getal")))
        }
        "num2str" => {
            arity(1)?;
            let q = a[0].as_quantity("het argument")?;
            if !q.is_dimensionless() {
                return Err(err(name, "werkt alleen met een getal zonder eenheid; deel eerst door de eenheid"));
            }
            Ok(Value::Str(format_number(q.value)))
        }
        "str2vec" => {
            arity(1)?;
            let codes: Vec<Quantity> = text(name, &a[0], "het argument")?.chars().map(|c| Quantity::scalar(c as u32 as f64)).collect();
            if codes.is_empty() {
                return Err(err(name, "een lege tekst geeft geen vector"));
            }
            Ok(Value::Matrix(Matrix::column(codes)))
        }
        "vec2str" => {
            arity(1)?;
            let elems: Vec<Quantity> = match &a[0] {
                Value::Matrix(m) => m.data.clone(),
                Value::Num(q) => vec![*q],
                other => return Err(err(name, &format!("verwacht een vector met tekencodes, geen {}", other.kind()))),
            };
            let mut s = String::new();
            for q in elems {
                let c = char::from_u32(q.value as u32).filter(|_| q.value.fract() == 0.0 && q.value >= 0.0);
                s.push(c.ok_or_else(|| err(name, &format!("{} is geen tekencode", q.value)))?);
            }
            Ok(Value::Str(s))
        }
        "IsString" => {
            arity(1)?;
            Ok(Value::bool(matches!(a[0], Value::Str(_))))
        }
        "error" => {
            arity(1)?;
            Err(Error::Domain(text(name, &a[0], "het argument")?.to_string()))
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use crate::Env;

    fn run(src: &str) -> String {
        let mut env = Env::new();
        let mut last = String::new();
        for line in src.lines() {
            if let Some(out) = env.run_line(line).unwrap_or_else(|e| panic!("{line}: {e}")) {
                last = out;
            }
        }
        last
    }

    #[test]
    fn tekstfuncties() {
        assert_eq!(run("s := concat(\"Hallo \", \"wereld\")\ns ="), "s = \"Hallo wereld\"");
        assert_eq!(run("strlen(\"abcd\") ="), "strlen(\"abcd\") = 4");
        assert_eq!(run("substr(\"MathPrime\", 4, 5) ="), "substr(\"MathPrime\", 4, 5) = \"Prime\"");
        assert_eq!(run("search(\"abcabc\", \"c\", 3) ="), "search(\"abcabc\", \"c\", 3) = 5");
        assert_eq!(run("search(\"abc\", \"x\", 0) ="), "search(\"abc\", \"x\", 0) = -1");
        assert_eq!(run("str2num(\"2.5\") * 2 ="), "str2num(\"2.5\") * 2 = 5");
        assert_eq!(run("num2str(3.5) ="), "num2str(3.5) = \"3.5\"");
        assert_eq!(run("vec2str(str2vec(\"ok\")) ="), "vec2str(str2vec(\"ok\")) = \"ok\"");
        assert_eq!(run("IsString(\"a\") + IsString(1) ="), "IsString(\"a\") + IsString(1) = 1");
        // vergelijken en een aanhalingsteken binnen een tekst
        assert_eq!(run("\"abc\" == \"abc\" ="), "\"abc\" == \"abc\" = 1");
        assert_eq!(run("t := \"zeg \"\"hoi\"\"\"\nstrlen(t) ="), "strlen(t) = 9");
    }

    #[test]
    fn rekenen_met_tekst_geeft_een_duidelijke_fout() {
        let mut env = Env::new();
        let e = env.run_line("\"a\" + 1 =").unwrap_err().to_string();
        assert!(e.contains("concat"), "{e}");
        let e = env.run_line("error(\"te groot\") =").unwrap_err().to_string();
        assert!(e.contains("te groot"), "{e}");
    }
}
