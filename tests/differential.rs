//! Binding-level differential: teptris::loads vs the toml crate over
//! the checked-in corpus. The engine's semantic differential (in the
//! teptris repo) proves the parse; this proves the BINDING's Value
//! materialization. Comparison rules:
//!   - nan normalizes on both sides
//!   - datetimes compare at MILLISECOND precision: the toml crate's
//!     Datetime Display carries only its own text precision (the
//!     engine's nanosecond fidelity is gated by the tomllib
//!     differential instead)
//! Zero divergences = pass.

use std::path::Path;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
enum Val {
    Str(String),
    Int(i64),
    Float(u64), // bit pattern: NaN-normalized before construction
    Bool(bool),
    Dt(String),
    Arr(Vec<Val>),
    Table(Vec<(String, Val)>),
}

fn ms3(n: u32) -> String {
    if n > 0 {
        format!(".{:03}", n / 1_000_000)
    } else {
        String::new()
    }
}

fn norm(v: &teptris::Value) -> Val {
    use teptris::Value as V;
    match v {
        V::String(s) => Val::Str(s.clone()),
        V::Integer(i) => Val::Int(*i),
        V::Float(f) if f.is_nan() => Val::Float(f64::NAN.to_bits()),
        V::Float(f) => Val::Float(f.to_bits()),
        V::Boolean(b) => Val::Bool(*b),
        V::Datetime(d) => Val::Dt(match d.kind {
            teptris::DatetimeKind::Time => format!(
                "T{:02}:{:02}:{:02}{}",
                d.hour,
                d.minute,
                d.second,
                ms3(d.nanosecond)
            ),
            teptris::DatetimeKind::Date => {
                format!("D{:04}-{:02}-{:02}", d.year, d.month, d.day)
            }
            teptris::DatetimeKind::LocalDateTime => format!(
                "D{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}",
                d.year,
                d.month,
                d.day,
                d.hour,
                d.minute,
                d.second,
                ms3(d.nanosecond)
            ),
            teptris::DatetimeKind::Offset => {
                let off = match d.offset_seconds {
                    Some(0) | None => "Z".to_string(),
                    Some(o) => {
                        let sign = if o < 0 { '-' } else { '+' };
                        let a = o.abs();
                        format!("{sign}{:02}:{:02}", a / 3600, (a % 3600) / 60)
                    }
                    _ => "Z".to_string(),
                };
                format!(
                    "D{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{}",
                    d.year,
                    d.month,
                    d.day,
                    d.hour,
                    d.minute,
                    d.second,
                    ms3(d.nanosecond),
                    off
                )
            }
        }),
        V::Array(a) => Val::Arr(a.iter().map(norm).collect()),
        V::Table(t) => {
            let mut pairs: Vec<(String, Val)> =
                t.iter().map(|(k, x)| (k.to_string(), norm(x))).collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            Val::Table(pairs)
        }
    }
}

// the toml crate's Datetime fields are private in 0.22: its RFC3339
// Display carries the precision the value holds; fractions truncate to
// the same millisecond key (and a zero fraction disappears - the
// engine-side ms3 already omits it)
fn dt_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'.' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let frac = &s[start..j];
            if frac.bytes().any(|c| c != b'0') {
                let mut digits = frac[..frac.len().min(3)].to_string();
                while digits.len() < 3 {
                    digits.push('0');
                }
                out.push('.');
                out.push_str(&digits);
            }
            i = j;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    if out.contains('-') && out.contains('T') {
        format!("D{out}")
    } else if out.contains(':') {
        format!("T{out}")
    } else {
        format!("D{out}")
    }
}

fn norm_toml(v: &toml::Value) -> Val {
    match v {
        toml::Value::String(s) => Val::Str(s.clone()),
        toml::Value::Integer(i) => Val::Int(*i),
        toml::Value::Float(f) if f.is_nan() => Val::Float(f64::NAN.to_bits()),
        toml::Value::Float(f) => Val::Float(f.to_bits()),
        toml::Value::Boolean(b) => Val::Bool(*b),
        toml::Value::Datetime(d) => Val::Dt(dt_key(&d.to_string())),
        toml::Value::Array(a) => Val::Arr(a.iter().map(norm_toml).collect()),
        toml::Value::Table(t) => {
            let mut pairs: Vec<(String, Val)> =
                t.iter().map(|(k, x)| (k.clone(), norm_toml(x))).collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            Val::Table(pairs)
        }
    }
}

fn eq(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::Arr(x), Val::Arr(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| eq(a, b)),
        (Val::Table(x), Val::Table(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|((ka, va), (kb, vb))| ka == kb && eq(va, vb))
        }
        _ => a == b,
    }
}

fn diff(a: &Val, b: &Val, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Val::Table(x), Val::Table(y)) => {
            for (k, va) in x {
                match y.iter().find(|(kb, _)| kb == k) {
                    Some((_, vb)) => diff(va, vb, &format!("{path}.{k}"), out),
                    None => out.push(format!("{path}.{k}: missing on toml side")),
                }
                if out.len() > 4 {
                    return;
                }
            }
            for (k, vb) in y {
                if !x.iter().any(|(ka, _)| ka == k) {
                    out.push(format!("{path}.{k}: missing on teptris side"));
                }
            }
        }
        (Val::Arr(x), Val::Arr(y)) => {
            if x.len() != y.len() {
                out.push(format!("{path}: len {} vs {}", x.len(), y.len()));
                return;
            }
            for (i, (xa, ya)) in x.iter().zip(y).enumerate() {
                diff(xa, ya, &format!("{path}[{i}]"), out);
            }
        }
        _ => {
            if !eq(a, b) {
                out.push(format!("{path}: teptris={a:?} toml={b:?}"));
            }
        }
    }
}

#[test]
fn differential_vs_toml_crate() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test/corpus");
    let mut fails = 0;
    let mut n = 0;
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("corpus dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "toml").unwrap_or(false))
        .collect();
    entries.sort();
    for path in entries {
        let src = std::fs::read_to_string(&path).expect("read corpus");
        let mine = norm(&teptris::loads(src.as_bytes()).expect("teptris parse"));
        let refv = norm_toml(&toml::from_str::<toml::Value>(&src).expect("toml parse"));
        let mut out = Vec::new();
        diff(&mine, &refv, "$", &mut out);
        n += 1;
        if !out.is_empty() {
            println!("DIVERGENCE in {path:?}:");
            for d in &out {
                println!("  {d}");
            }
            fails += 1;
        }
    }
    println!("differential: {n} files, {fails} divergences");
    assert_eq!(fails, 0, "differential divergences");
}
