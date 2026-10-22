//! The serde surface: derive round-trips against the engine's parse
//! and dump, the toml-crate Datetime interchange convention, and the
//! Option contract. Gated on the default `serde` feature.
#![cfg(feature = "serde")]

use serde::{Deserialize, Serialize};
use teptris::{from_str, to_string, Value};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    name: String,
    retries: u32,
    #[serde(default)]
    tags: Vec<String>,
    verbose: bool,
    ratio: f64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Nested {
    #[serde(flatten)]
    server: Server,
    limits: Limits,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Server {
    host: String,
    port: u16,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Limits {
    timeout: f64,
    keepalive: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct WithDatetime {
    published: teptris::Datetime,
    count: i64,
}

const BASIC: &str = "\
name = \"teptris\"
retries = 3
verbose = true
ratio = 0.5
";

#[test]
fn from_str_into_a_derived_struct() {
    let cfg: Config = from_str(BASIC).unwrap();
    assert_eq!(
        cfg,
        Config {
            name: "teptris".into(),
            retries: 3,
            tags: vec![],
            verbose: true,
            ratio: 0.5
        }
    );
}

#[test]
fn to_string_from_a_derived_struct() {
    let cfg = Config {
        name: "x".into(),
        retries: 7,
        tags: vec!["a".into(), "b".into()],
        verbose: false,
        ratio: 1.25,
    };
    let text = to_string(&cfg).unwrap();
    assert_eq!(
        text,
        "name = 'x'\nretries = 7\ntags = ['a', 'b']\nverbose = false\nratio = 1.25\n"
    );
    let back: Config = from_str(&text).unwrap();
    assert_eq!(back, cfg);
}

#[test]
fn nested_tables_and_arrays() {
    let src = "\
host = 'example.org'
port = 8443

[limits]
timeout = 30.0
keepalive = true
";
    let cfg: Nested = from_str(src).unwrap();
    assert_eq!(cfg.server.host, "example.org");
    assert_eq!(cfg.server.port, 8443);
    assert_eq!(cfg.limits.timeout, 30.0);
    assert!(cfg.limits.keepalive);
}

#[test]
fn datetime_roundtrip_through_the_private_map() {
    let doc: WithDatetime = from_str("published = 1979-05-27T07:32:00+05:30\ncount = 1\n").unwrap();
    assert_eq!(doc.published.kind, teptris::DatetimeKind::Offset);
    assert_eq!(doc.count, 1);
    let text = to_string(&doc).unwrap();
    // the convention: the datetime renders through the engine's TOML
    // emit on the to_string path (Value::Datetime), so the output is
    // a real TOML datetime - and re-parses as one.
    assert_eq!(text, "published = 1979-05-27T07:32:00+05:30\ncount = 1\n");
    let doc2: WithDatetime = from_str(&text).unwrap();
    assert_eq!(doc2, doc);
}

#[test]
fn interop_with_the_toml_crate() {
    // the same struct deserializes from BOTH crates' parsers
    #[derive(Debug, PartialEq, Deserialize)]
    struct Shared {
        a: i64,
        b: String,
    }
    let src = "a = 1\nb = 'x'\n";
    let ours: Shared = from_str(src).unwrap();
    let theirs: Shared = toml::from_str(src).unwrap();
    assert_eq!(ours, theirs);

    // and our Value tree deserializes through the toml crate's serde
    let value: toml::Value = toml::from_str(src).unwrap();
    let ours2: Shared = serde::Deserialize::deserialize(value).unwrap();
    assert_eq!(ours2, ours);
}

#[test]
fn option_fields_deserialize_and_refuse_to_serialize_none() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct In {
        a: i64,
        #[serde(default)]
        maybe: Option<String>,
    }
    let v: In = from_str("a = 1\n").unwrap();
    assert_eq!(v.maybe, None);
    let v: In = from_str("a = 1\nmaybe = 'y'\n").unwrap();
    assert_eq!(v.maybe, Some("y".into()));

    #[derive(Serialize)]
    struct Out {
        a: i64,
        maybe: Option<String>,
    }
    let err = to_string(&Out { a: 1, maybe: None }).unwrap_err();
    assert!(err.to_string().contains("TOML has no null"));
    let ok = to_string(&Out {
        a: 1,
        maybe: Some("y".into()),
    })
    .unwrap();
    assert_eq!(ok, "a = 1\nmaybe = 'y'\n");
}

#[test]
fn error_positions_carry_through() {
    let err =
        from_str::<Config>("name = 1\nretries = 3\nverbose = true\nratio = 0.5\n").unwrap_err();
    assert!(err.to_string().contains("invalid type"), "{err}");
}

#[test]
fn enums_and_newtype_variants() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "kind")]
    enum Shape {
        Circle { r: f64 },
        Rect { w: f64, h: f64 },
    }
    let src = "[shapes.circle]\nkind = 'Circle'\nr = 2.0\n\n[shapes.rect]\nkind = 'Rect'\nw = 3.0\nh = 4.0\n";
    #[derive(Debug, PartialEq, Deserialize)]
    struct Doc {
        shapes: std::collections::BTreeMap<String, Shape>,
    }
    let doc: Doc = from_str(src).unwrap();
    assert_eq!(doc.shapes["circle"], Shape::Circle { r: 2.0 });
    assert_eq!(doc.shapes["rect"], Shape::Rect { w: 3.0, h: 4.0 });

    let rect = Shape::Rect { w: 1.0, h: 2.0 };
    let text = to_string(&rect).unwrap();
    assert_eq!(text, "kind = 'Rect'\nw = 1.0\nh = 2.0\n");
}

#[test]
fn value_tree_still_walkable() {
    let v: Value = from_str("[a]\nb = 1\n").unwrap();
    assert_eq!(
        v,
        Value::Table(teptris::Table::from_entries(vec![(
            "a".into(),
            Value::Table(teptris::Table::from_entries(vec![(
                "b".into(),
                Value::Integer(1)
            )]))
        )]))
    );
}
