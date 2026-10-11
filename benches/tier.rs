//! The rust tier: teptris-rs vs the ecosystem's TOML crate over the
//! engine bench corpus — the last unmeasured binding tier.
//! Comparators: `toml` (toml-rs 0.8, the standard).
//!   cargo bench --bench tier -- corpus-dir reps
use std::fs;
use std::path::Path;
use std::time::Instant;

fn best_of<F: FnMut()>(mut f: F, reps: u32) -> f64 {
    f(); // warmup
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t0 = Instant::now();
        f();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        if ms < best {
            best = ms;
        }
    }
    best
}

fn fmt(v: Option<f64>) -> String {
    match v {
        None => String::from("   n/a"),
        Some(x) => format!("{x:>7.2}"),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let corpus = args.next().unwrap_or_else(|| "corpus".into());
    let reps: u32 = args.next().map(|s| s.parse().unwrap()).unwrap_or(40);

    let mut shapes: Vec<_> = fs::read_dir(&corpus)
        .expect("corpus dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "toml").unwrap_or(false))
        .collect();
    shapes.sort();

    println!("shape                 loads            dumps -> TOML    (ms, best-of-{reps})");
    println!("shape                 teptris    toml   teptris    toml");
    for path in shapes {
        let src = fs::read_to_string(&path).expect("read");
        let name = Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let tep_loads = {
            let bytes = src.as_bytes();
            best_of(
                || {
                    teptris::loads(bytes).expect("teptris parse");
                },
                reps,
            )
        };
        let toml_loads = {
            let s = src.clone();
            best_of(
                move || {
                    toml::from_str::<toml::Value>(&s).expect("toml parse");
                },
                reps,
            )
        };

        let tep_val = teptris::loads(src.as_bytes()).unwrap();
        let tep_dump = best_of(
            || {
                teptris::dump(&tep_val).expect("teptris dump");
            },
            reps,
        );
        let toml_val: toml::Value = toml::from_str(&src).unwrap();
        let toml_dump = best_of(
            || {
                toml::to_string(&toml_val).expect("toml dump");
            },
            reps,
        );

        println!(
            "{name:<20} {} {} {} {}",
            fmt(Some(tep_loads)),
            fmt(Some(toml_loads)),
            fmt(Some(tep_dump)),
            fmt(Some(toml_dump)),
        );
    }
}
