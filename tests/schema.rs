//! The schema surface: compile-once/validate-many, typed violations,
//! compile-time rejection of unsupported constructs, and the
//! policies' behavior from the Rust side. Semantics are owned by the
//! engine's unit suite; this file keeps the binding honest.

use teptris::schema::{Schema, SchemaCompileError};
use teptris::Document;

const SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "name": {"type": "string", "minLength": 1},
    "port": {"type": "integer", "minimum": 1}
  },
  "required": ["name", "port"]
}"#;

#[test]
fn compiles_and_validates() {
    let schema = Schema::compile(SCHEMA).unwrap();
    let ok = schema.validate(b"name = \"srv\"\nport = 8080\n").unwrap();
    assert!(ok.is_empty());

    let bad = schema.validate(b"name = \"\"\nport = 0\n").unwrap();
    assert_eq!(bad.len(), 2);
    assert_eq!(bad[0].path, "name");
    assert_eq!(bad[0].schema_path, "#/properties.name");
    assert_eq!(bad[1].path, "port");

    assert_eq!(bad[0].message, "string is shorter than minLength");
    assert_eq!(bad[1].message, "value is below the minimum");

    let missing = schema.validate(b"port = 1\n").unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].path, "");
    assert!(missing[0].message.contains("name"));

    assert!(schema.is_valid("name = \"a\"\nport = 1\n"));
    assert!(!schema.is_valid("port = 1\n"));
}

#[test]
fn rejects_unsupported_constructs_at_compile() {
    let err: SchemaCompileError =
        Schema::compile(r#"{"unevaluatedProperties":false}"#).unwrap_err();
    assert!(err.diag.contains("unsupported"), "{}", err.diag);

    let err = Schema::compile(r#"{"$ref":"https://x.example/s.json"}"#).unwrap_err();
    assert!(err.diag.contains("remote"), "{}", err.diag);

    let err = Schema::compile(r#"{"type":}"#).unwrap_err();
    assert!(err.diag.contains("error"), "{}", err.diag);
}

#[test]
fn unknown_keywords_are_extensions() {
    let schema =
        Schema::compile(r#"{"x-vendor":true,"properties":{"k":{"type":"integer"}}}"#).unwrap();
    assert!(schema.is_valid("k = 1\n"));
    assert!(!schema.is_valid("k = \"s\"\n"));
}

#[test]
fn policies_hold_from_rust() {
    // policy 2: a float never validates as integer; enum is type-strict
    let schema = Schema::compile(r#"{"properties":{"k":{"enum":[1]}}}"#).unwrap();
    assert!(schema.is_valid("k = 1\n"));
    assert!(!schema.is_valid("k = 1.0\n"));

    // policy 3: datetimes are strings with formats
    let schema =
        Schema::compile(r#"{"properties":{"k":{"type":"string","format":"date-time"}}}"#).unwrap();
    assert!(schema.is_valid("k = 2026-01-02T03:04:05Z\n"));
    assert!(!schema.is_valid("k = \"soon\"\n"));

    // policy 4: nan fails every ordered comparison
    let schema = Schema::compile(r#"{"properties":{"k":{"minimum":0}}}"#).unwrap();
    assert!(!schema.is_valid("k = nan\n"));
    assert!(schema.is_valid("k = inf\n"));
}

#[test]
fn recursive_refs_memoize() {
    let schema = Schema::compile(
        r##"{"properties":{"v":{"type":"integer"},"kids":{"type":"array","items":{"$ref":"#"}}},"required":["v"]}"##,
    )
    .unwrap();
    assert!(schema.is_valid("v = 0\n[[kids]]\nv = 1\n[[kids.kids]]\nv = 2\n"));
    let bad = schema.validate(b"v = 0\n[[kids]]\nv = \"x\"\n").unwrap();
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].path, "kids.0.v");
}

#[test]
fn syntax_errors_are_statuses_not_violations() {
    let schema = Schema::compile("{}").unwrap();
    let err = schema.validate(b"a = = 1\n").unwrap_err();
    assert_eq!(err.0, teptris::TEPTRIS_ERR_SYNTAX);
}

#[test]
fn validate_doc_reuses_a_parse() {
    let schema = Schema::compile(SCHEMA).unwrap();
    let doc = Document::parse(b"name = \"a\"\nport = 1\n").unwrap();
    assert!(schema.validate_doc(&doc).unwrap().is_empty());
    let doc = Document::parse(b"port = \"x\"\n").unwrap();
    let bad = schema.validate_doc(&doc).unwrap();
    assert_eq!(bad.len(), 2);
}

#[test]
fn handle_is_send_and_drops_cleanly() {
    fn assert_send<T: Send>() {}
    assert_send::<Schema>();
    let schema = std::sync::Arc::new(Schema::compile(SCHEMA).unwrap());
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let s = std::sync::Arc::clone(&schema);
            std::thread::spawn(move || {
                let toml = format!("name = \"a\"\nport = {}\n", 8080 + i);
                assert!(s.validate(toml.as_bytes()).unwrap().is_empty());
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
}
