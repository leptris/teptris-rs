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
    // unevaluatedProperties is enforced since engine 0.3.22; the
    // out-of-subset constructs still name themselves at compile
    let enforced = Schema::compile(r#"{"unevaluatedProperties":false}"#).unwrap();
    assert_eq!(enforced.validate(b"a = 1\n").unwrap().len(), 1);

    let err: SchemaCompileError =
        Schema::compile(r#"{"$ref":"https://x.example/s.json"}"#).unwrap_err();
    assert!(err.diag.contains("remote"), "{}", err.diag);

    let err = Schema::compile(r#"{"$id":"https://example.com/a","allOf":[{"$ref":"other"}]}"#)
        .unwrap_err();
    assert!(
        err.diag.contains("remote") || err.diag.contains("unresolvable"),
        "{}",
        err.diag
    );

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
    // policy 2: enum equality is 2020-12 numeric identity since engine
    // 0.3.23 (enum [1] matches 1.0, still rejects 1.5)
    let schema = Schema::compile(r#"{"properties":{"k":{"enum":[1]}}}"#).unwrap();
    assert!(schema.is_valid("k = 1\n"));
    assert!(schema.is_valid("k = 1.0\n"));
    assert!(!schema.is_valid("k = 1.5\n"));

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

#[test]
fn the_taplo_parity_surface_rides_engine_v0_3_21() {
    // draft-07 dependencies: the array form is a dependentRequired pair
    let deps = Schema::compile(r#"{"dependencies":{"credit_card":["billing_address"]}}"#).unwrap();
    assert_eq!(deps.validate(b"a = 1\ncredit_card = 1\n").unwrap().len(), 1);
    assert!(deps
        .validate(b"credit_card = 1\nbilling_address = 'x'\n")
        .unwrap()
        .is_empty());

    // tuple items + additionalItems:false
    let tuple = Schema::compile(
        r#"{"properties":{"t":{"items":[{"type":"string"}],"additionalItems":false}}}"#,
    )
    .unwrap();
    assert!(tuple.validate(b"t = ['x']\n").unwrap().is_empty());
    assert_eq!(tuple.validate(b"t = ['x', 1]\n").unwrap().len(), 1);

    // exact-decimal multipleOf: 0.3 is a multiple of 0.1
    let mult = Schema::compile(r#"{"properties":{"k":{"multipleOf":0.1}}}"#).unwrap();
    assert!(mult.validate(b"k = 0.3\n").unwrap().is_empty());
    assert_eq!(mult.validate(b"k = 0.35\n").unwrap().len(), 1);

    // the enforced format set: semver-requirement rides taplo's own
    // custom formats, uuid is unreachable in taplo's build
    let req = Schema::compile(r#"{"properties":{"k":{"format":"semver-requirement"}}}"#).unwrap();
    assert!(req.validate(b"k = '^1.2'\n").unwrap().is_empty());
    assert_eq!(req.validate(b"k = 'not a req'\n").unwrap().len(), 1);

    let uuid = Schema::compile(r#"{"properties":{"k":{"format":"uuid"}}}"#).unwrap();
    assert!(uuid
        .validate(b"k = '123e4567-e89b-12d3-a456-426614174000'\n")
        .unwrap()
        .is_empty());
    assert_eq!(uuid.validate(b"k = 'nope'\n").unwrap().len(), 1);
}

#[test]
fn the_sota_surface_rides_engine_v0_3_22() {
    // unevaluatedProperties with the cousin rule
    let cousin =
        Schema::compile(r#"{"allOf":[{"properties":{"a":{}}},{"unevaluatedProperties":false}]}"#)
            .unwrap();
    assert_eq!(cousin.validate(b"a = 1\n").unwrap().len(), 1);

    // the same-object adjacency DOES hold
    let nested =
        Schema::compile(r#"{"properties":{"a":{}},"unevaluatedProperties":false}"#).unwrap();
    assert!(nested.validate(b"a = 1\n").unwrap().is_empty());
    assert_eq!(nested.validate(b"a = 1\nb = 2\n").unwrap().len(), 1);

    // $dynamicRef: the innermost anchor wins
    let tree = Schema::compile(
        r##"{"$dynamicAnchor":"node","type":"object","properties":{
            "value":{"type":"integer"},
            "children":{"type":"array","items":{"$dynamicRef":"#node"}}}}"##,
    )
    .unwrap();
    assert!(tree
        .validate(b"value = 1\n[[children]]\nvalue = 2\n")
        .unwrap()
        .is_empty());
    assert_eq!(
        tree.validate(b"value = 1\n[[children]]\nvalue = 'x'\n")
            .unwrap()
            .len(),
        1
    );

    // contentSchema asserts the decoded content
    let content = Schema::compile(
        r#"{"properties":{"k":{"contentEncoding":"base64",
            "contentMediaType":"application/json",
            "contentSchema":{"type":"object","required":["n"]}}}}"#,
    )
    .unwrap();
    assert!(content
        .validate(b"k = 'eyJuIjogMX0='\n")
        .unwrap()
        .is_empty());
    assert_eq!(content.validate(b"k = 'W10='\n").unwrap().len(), 1);

    // tombi's TOML-native local formats
    let local = Schema::compile(r#"{"properties":{"k":{"format":"time-local"}}}"#).unwrap();
    assert!(local.validate(b"k = 07:32:00\n").unwrap().is_empty());
    assert_eq!(local.validate(b"k = '07:32:00Z'\n").unwrap().len(), 1);
}

#[test]
fn builtin_metaschemas_resolve_in_memory() {
    // The vendored metaschemas ship as built-in resources since
    // engine 0.3.23: metaschema refs never touch the network.
    let schema =
        Schema::compile(r#"{"$ref":"https://json-schema.org/draft/2020-12/schema"}"#).unwrap();
    assert!(schema
        .validate(
            br#"type = "string"
"#
        )
        .unwrap()
        .is_empty());
    assert_eq!(
        schema
            .validate(
                br#"type = 123
"#
            )
            .unwrap()
            .len(),
        1
    );

    // the legacy draft-07 URL with the empty-fragment form
    let d7 = Schema::compile(r#"{"$ref":"http://json-schema.org/draft-07/schema#"}"#).unwrap();
    assert!(d7
        .validate(
            br#"description = "d"
"#
        )
        .unwrap()
        .is_empty());
}

#[test]
fn unicode_property_escapes() {
    // \p{...} since engine 0.3.24: generated category tables
    let letter = Schema::compile(r#"{"properties":{"k":{"pattern":"^\\p{Letter}+$"}}}"#).unwrap();
    assert!(letter
        .validate("k = \"\u{3c0}\"\n".as_bytes())
        .unwrap()
        .is_empty());
    assert_eq!(
        letter
            .validate(
                br#"k = "123"
"#
            )
            .unwrap()
            .len(),
        1
    );
    let neg = Schema::compile(r#"{"properties":{"k":{"pattern":"^\\P{Letter}+$"}}}"#).unwrap();
    assert!(neg
        .validate(
            br#"k = "123"
"#
        )
        .unwrap()
        .is_empty());
}

#[test]
fn dialect_gated_formats() {
    // engine 0.3.29: a declared dialect annotates format/content*;
    // the assertion dialect opts back in
    let plain = Schema::compile(r#"{"properties":{"k":{"format":"email"}}}"#).unwrap();
    assert_eq!(
        plain
            .validate(
                br#"k = "2962"
"#
            )
            .unwrap()
            .len(),
        1
    );
    let anno = Schema::compile(
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","properties":{"k":{"format":"email"}}}"#,
    )
    .unwrap();
    assert!(anno
        .validate(
            br#"k = "2962"
"#
        )
        .unwrap()
        .is_empty());
    let assertion = Schema::compile(
        r#"{"$schema":"https://json-schema.org/draft/2020-12/format-assertion","properties":{"k":{"format":"email"}}}"#,
    )
    .unwrap();
    assert_eq!(
        assertion
            .validate(
                br#"k = "2962"
"#
            )
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn compiles_long_plain_tail_after_escape() {
    // engine 0.3.26: the schema JSON reader sized the escaped-string
    // rebuild from the plain prefix alone; a long plain tail after the
    // first escape wrote past the arena block (fuzz finding)
    let mut json = String::from("{\"description\":\"A\\u00e9");
    json.push_str(&"B".repeat(128 * 1024));
    json.push_str("\"}");
    let schema = Schema::compile(&json).unwrap();
    assert_eq!(
        schema
            .validate(
                br#"k = 1
"#
            )
            .unwrap()
            .len(),
        0
    );
}
