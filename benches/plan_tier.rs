//! The plan tier (the descriptor's measurement half): plan-direct
//! extraction vs the full-Value and serde-derive paths over the SAME
//! struct-shaped document serde_tier uses. The descriptor compiles
//! once (untimed) and each rep pays parse + one planned walk.
//! Report-only: the verdict lands in benchmarks/LEDGER.md, dated.
//!
//! Run: cargo bench --bench plan_tier --features serde
use serde::{Deserialize, Serialize};
use std::time::Instant;
use teptris::plan::{Descriptor, PlanRow};

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

fn generate(sections: usize) -> String {
    let mut out = String::with_capacity(sections * 1300 + 128);
    out.push_str("[meta]\ntitle = 'plan tier'\nrevision = 7\ndraft = false\n");
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

/// The full Doc schema as a plan: every field the derive targets.
fn full_schema() -> Vec<PlanRow> {
    vec![
        PlanRow::nested(
            "meta",
            vec![
                PlanRow::scalar("title"),
                PlanRow::scalar("revision"),
                PlanRow::scalar("draft"),
            ],
        ),
        PlanRow::nested(
            "section",
            vec![
                PlanRow::scalar("name"),
                PlanRow::scalar("weight"),
                PlanRow::scalar("ratio"),
                PlanRow::scalar("enabled"),
                PlanRow::collection("tags"),
                PlanRow::scalar("endpoint"),
                PlanRow::scalar("retries"),
                PlanRow::scalar("note"),
            ],
        ),
    ]
}

/// A quarter of the schema: proves savings scale with planned breadth.
fn partial_schema() -> Vec<PlanRow> {
    vec![PlanRow::nested(
        "section",
        vec![PlanRow::scalar("name"), PlanRow::scalar("retries")],
    )]
}

/// Half the schema: bounds the plan-vs-derive crossover.
fn half_schema() -> Vec<PlanRow> {
    vec![PlanRow::nested(
        "section",
        vec![
            PlanRow::scalar("name"),
            PlanRow::scalar("ratio"),
            PlanRow::collection("tags"),
            PlanRow::scalar("note"),
        ],
    )]
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
    println!("{label:<52} {best_ms:>10.2} ms");
    best_ms
}

fn main() {
    let src = generate(2000);
    let bytes = src.as_bytes();
    println!(
        "plan tier: struct-shaped doc, 2000 sections, {} KB, best-of-20\n",
        src.len() / 1024
    );

    let parse_ms = bench("engine  Document::parse (floor)", 20, || {
        let d = teptris::Document::parse(bytes).unwrap();
        std::hint::black_box(&d);
    });

    let value_ms = bench("engine  loads -> full Value", 20, || {
        let v = teptris::loads(bytes).unwrap();
        std::hint::black_box(&v);
    });

    let plan_full = Descriptor::build(&full_schema()).unwrap();
    let plan_part = Descriptor::build(&partial_schema()).unwrap();
    let plan_half = Descriptor::build(&half_schema()).unwrap();

    // Correctness gate: the full plan must recover the same shape the
    // derive produces (200 sections x 8 fields + 3-field meta).
    {
        let d = teptris::Document::parse(bytes).unwrap();
        let v = plan_full.walk(&d).unwrap();
        let t = v.as_table().unwrap();
        assert_eq!(t.get("meta").unwrap().as_table().unwrap().len(), 3);
        let secs = t.get("section").unwrap().as_array().unwrap();
        assert_eq!(secs.len(), 2000);
        assert_eq!(secs[0].as_table().unwrap().len(), 8);
        assert_eq!(
            secs[0].as_table().unwrap().get("name").unwrap().as_str(),
            Some("section-0")
        );
    }

    let plan_full_ms = bench("engine  parse + plan walk (full schema)", 20, || {
        let d = teptris::Document::parse(bytes).unwrap();
        let v = plan_full.walk(&d).unwrap();
        std::hint::black_box(&v);
    });

    let plan_part_ms = bench("engine  parse + plan walk (2 of 8 fields)", 20, || {
        let d = teptris::Document::parse(bytes).unwrap();
        let v = plan_part.walk(&d).unwrap();
        std::hint::black_box(&v);
    });

    let plan_half_ms = bench("engine  parse + plan walk (4 of 8 fields)", 20, || {
        let d = teptris::Document::parse(bytes).unwrap();
        let v = plan_half.walk(&d).unwrap();
        std::hint::black_box(&v);
    });

    bench("engine  parse + borrowed walk (headline reads)", 20, || {
        let doc2 = teptris::Document::parse(bytes).unwrap();
        let r = plan_full.walk_borrowed(&doc2).unwrap();
        let t = r.table("meta").unwrap();
        std::hint::black_box(t.get_str("title"));
        std::hint::black_box(r.aot_len("section"));
        std::hint::black_box(&r);
    });

    let derive_ms = bench("engine  from_str::<Doc> (serde derive)", 20, || {
        let doc: Doc = teptris::from_str(&src).unwrap();
        std::hint::black_box(&doc);
    });

    let toml_ms = bench("toml    from_str::<Doc> (serde derive)", 20, || {
        let doc: Doc = toml::from_str(&src).unwrap();
        std::hint::black_box(&doc);
    });

    println!("\nratios (vs engine serde derive)");
    for (label, ms) in [
        ("parse floor", parse_ms),
        ("full Value", value_ms),
        ("plan full schema", plan_full_ms),
        ("plan half (4/8)", plan_half_ms),
        ("plan partial (2/8)", plan_part_ms),
        ("serde derive", derive_ms),
    ] {
        println!(
            "  {label:<20} {derive_ms:>8.2} / {ms:>8.2} = {:>5.2}x",
            derive_ms / ms
        );
    }
    println!("\nvs toml crate derive: {:.2}x", toml_ms / plan_full_ms);
}
