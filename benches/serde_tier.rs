//! The serde tier (B2's measurement half): derive-deserialization
//! head-to-head — `teptris::from_str` vs `toml::from_str` over the
//! same struct-shaped document (the schema both crates' derives
//! target), plus the Value-model reference cell from benches/tier.rs.
//! Report-only: the verdict lands in benchmarks/LEDGER.md, dated.
//!
//! Run: cargo bench --bench serde_tier
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize)]
struct Doc {
    meta: Meta,
    #[serde(rename = "section")]
    sections: Vec<Section>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Meta {
    title: String,
    revision: i64,
    draft: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Section {
    name: String,
    weight: i64,
    ratio: f64,
    enabled: bool,
    tags: Vec<String>,
    endpoint: String,
    retries: i64,
    note: String,
}

/// Deterministic struct-shaped document: 200 `[[section]]` array
/// entries x 8 typed fields, plus a 3-field meta table (~400 KB at
/// 2000 sections).
fn generate(sections: usize) -> String {
    let mut out = String::with_capacity(sections * 1300 + 128);
    out.push_str("[meta]\ntitle = 'serde tier'\nrevision = 7\ndraft = false\n");
    for i in 0..sections {
        out.push_str(&format!(
            "[[section]]\n\
             name = 'section-{i}'\n\
             weight = {i}\n\
             ratio = {r:.3}\n\
             enabled = {en}\n\
             tags = ['a{i}', 'b', 'c']\n\
             endpoint = 'https://example.org/endpoint/{i}'\n\
             retries = {ret}\n\
             note = 'note for section {i}: the quick brown fox'\n",
            r = (i % 1000) as f64 / 1000.0,
            en = i % 2 == 0,
            ret = i % 7,
        ));
    }
    out
}

fn bench<F: FnMut()>(label: &str, reps: u32, mut f: F) -> f64 {
    f(); // warm-up
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t0 = Instant::now();
        f();
        let dt = t0.elapsed().as_secs_f64();
        if dt < best {
            best = dt;
        }
    }
    let best_ms = best * 1000.0;
    println!("{label:<44} {best_ms:>10.2} ms");
    best_ms
}

fn main() {
    let src = generate(2000);
    let bytes = src.len();
    println!(
        "serde tier: struct-shaped doc, {} sections, {} KB, best-of-20\n",
        2000,
        bytes / 1024
    );

    let t_teptris = bench("teptris  from_str::<Doc> (derive)", 20, || {
        let doc: Doc = teptris::from_str(&src).unwrap();
        std::hint::black_box(&doc);
    });
    let t_toml = bench("toml     from_str::<Doc> (derive)", 20, || {
        let doc: Doc = toml::from_str(&src).unwrap();
        std::hint::black_box(&doc);
    });
    println!(
        "{:<44} {:>10.2}\n",
        format!("-> teptris/toml derive ratio"),
        t_toml / t_teptris
    );

    // the Value-model reference (the tier.rs cell, same doc)
    let t_v_tepris = bench("teptris  loads -> Value", 20, || {
        let v: teptris::Value = teptris::loads(src.as_bytes()).unwrap();
        std::hint::black_box(&v);
    });
    let t_v_toml = bench("toml     from_str -> toml::Value", 20, || {
        let v: toml::Value = toml::from_str(&src).unwrap();
        std::hint::black_box(&v);
    });
    println!(
        "{:<44} {:>10.2}\n",
        format!("-> teptris/toml Value ratio"),
        t_v_toml / t_v_tepris
    );

    // serialize side: struct -> TOML text
    let doc: Doc = teptris::from_str(&src).unwrap();
    let t_ser = bench("teptris  to_string(&Doc)", 20, || {
        let s = teptris::to_string(&doc).unwrap();
        std::hint::black_box(&s);
    });
    let toml_doc: toml::Value = toml::from_str(&src).unwrap();
    let t_toml_ser = bench("toml     to_string(&Doc)", 20, || {
        let s = toml::to_string(&toml_doc).unwrap();
        std::hint::black_box(&s);
    });
    println!(
        "{:<44} {:>10.2}\n",
        format!("-> teptris/toml serialize ratio"),
        t_toml_ser / t_ser
    );
}
