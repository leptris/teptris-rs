//! Round-trip and contract tests over the real shared library.

use teptris::{dump, dump_json, loads, Datetime, DatetimeKind, Value};

#[test]
fn loads_scalars() {
    let v = loads(b"name = \"demo\"\nport = 8080\nratio = 1.5\non = true\n").unwrap();
    assert_eq!(v.get("name").and_then(Value::as_str), Some("demo"));
    assert_eq!(v.get("port").and_then(Value::as_integer), Some(8080));
    assert_eq!(v.get("ratio").and_then(Value::as_float), Some(1.5));
    assert_eq!(v.get("on").and_then(Value::as_boolean), Some(true));
}

#[test]
fn loads_preserves_table_order() {
    let v = loads(b"zeta = 1\nalpha = 2\nmid = 3\n").unwrap();
    let t = v.as_table().unwrap();
    let keys: Vec<&str> = t.iter().map(|(k, _)| k).collect();
    assert_eq!(keys, ["zeta", "alpha", "mid"]);
}

#[test]
fn loads_nested_and_arrays() {
    let v = loads(
        b"a = 1\n[t]\nk = 1.5\narr = [1, \"two\", [3]]\n[[items]]\nid = 1\n[[items]]\nid = 2\n",
    )
    .unwrap();
    let t = v.get("t").and_then(Value::as_table).unwrap();
    assert_eq!(t.get("k").and_then(Value::as_float), Some(1.5));
    let arr = t.get("arr").and_then(Value::as_array).unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[2].as_array().unwrap()[0].as_integer(), Some(3));
    let items = v.get("items").and_then(Value::as_array).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[1].as_table().unwrap().get("id").unwrap().as_integer(),
        Some(2)
    );
}

#[test]
fn loads_datetimes() {
    let v = loads(
        b"o = 1979-05-27T07:32:00-07:00\nl = 1979-05-27T07:32:00\nd = 1979-05-27\nt = 07:32:00\n",
    )
    .unwrap();
    let o = match v.get("o") {
        Some(Value::Datetime(dt)) => dt,
        other => panic!("expected offset datetime, got {:?}", other),
    };
    assert_eq!(o.kind, DatetimeKind::Offset);
    assert_eq!(o.year, 1979);
    assert_eq!(o.offset_seconds, Some(-25200));
    for (key, kind) in [
        ("l", DatetimeKind::LocalDateTime),
        ("d", DatetimeKind::Date),
        ("t", DatetimeKind::Time),
    ] {
        let dt = match v.get(key) {
            Some(Value::Datetime(dt)) => dt,
            other => panic!("expected datetime for {key}, got {:?}", other),
        };
        assert_eq!(dt.kind, kind);
        assert_eq!(dt.offset_seconds, None);
    }
}

#[test]
fn loads_unicode() {
    let v = loads("s = \"caf\u{e9} \u{2615}\"\n".as_bytes()).unwrap();
    assert_eq!(
        v.get("s").and_then(Value::as_str),
        Some("caf\u{e9} \u{2615}")
    );
}

#[test]
fn parse_error_carries_position() {
    let err = loads(b"a = 1\nbogus =\n").unwrap_err();
    assert!(err.message.contains("value"), "{}", err.message);
    assert_eq!(err.line, 2);
    assert!(err.column >= 8);
}

#[test]
fn roundtrip_through_dump() {
    let src = b"name = \"rt\"\nport = 8080\nneg = -12\nratio = 0.25\non = false\n[t]\nk = 1.5\n[[items]]\nid = 1\nname = \"one\"\n[[items]]\nid = 2\nname = \"two\"\n";
    let v = loads(src).unwrap();
    let toml = dump(&v).unwrap();
    let v2 = loads(toml.as_bytes()).unwrap();
    assert_eq!(v, v2, "parse(dump(v)) must equal v");
}

#[test]
fn roundtrip_datetimes_through_dump() {
    let src =
        b"o = 1979-05-27T07:32:00-07:00\nd = 1979-05-27\nt = 07:32:00\nl = 1979-05-27T07:32:00\n";
    let v = loads(src).unwrap();
    let toml = dump(&v).unwrap();
    assert_eq!(loads(toml.as_bytes()).unwrap(), v);
}

#[test]
fn emit_json_is_conformance_shape() {
    let doc_toml = b"a = 1\n";
    let json = teptris::Document::parse(doc_toml)
        .unwrap()
        .to_json_string()
        .unwrap();
    assert!(json.contains("\"type\":\"integer\""), "{json}");
    assert!(json.contains("\"value\":\"1\""), "{json}");
}

#[test]
fn document_toml_string_is_canonical() {
    let doc = teptris::Document::parse(b"b = 2\na = 1\n").unwrap();
    let toml = doc.to_toml_string().unwrap();
    // canonical emit: parse(emit(d)) == d, insertion order kept
    assert!(
        toml.contains("b = 2\na = 1") || toml.contains("b = 2\r\na = 1"),
        "{toml}"
    );
}

#[test]
fn root_table_helpers() {
    let v = loads(b"x = 1\n").unwrap();
    let t = v.as_table().unwrap();
    assert_eq!(t.len(), 1);
    assert!(!t.is_empty());
}

#[test]
fn engine_version_reports() {
    // CI builds against engine main: never hard-code the minor here
    // (a stale "0.1." prefix once stopped a release lane on 0.3.0)
    let v = teptris::engine_version();
    assert!(v.split('.').count() >= 3, "engine version: {v}");
}

#[test]
fn dump_json_natural_is_host_json() {
    let v = loads(
        b"i = 42\nf = 3.5\nt = true\ns = \"hi\"\nat = 2026-10-09T12:00:00Z\nnan = nan\ninf = inf\n",
    )
    .unwrap();
    let json = teptris::dump_json_natural(&v).unwrap();
    // real numbers and booleans (no tagged "type"/"value" wrapping)
    assert!(json.contains("\"i\":42"), "{json}");
    assert!(json.contains("\"f\":3.5"), "{json}");
    assert!(json.contains("\"t\":true"), "{json}");
    // datetime as an RFC 3339 string, non-finite floats as null
    assert!(json.contains("\"at\":\"2026-10-09T12:00:00Z\""), "{json}");
    assert!(json.contains("\"nan\":null"), "{json}");
    assert!(json.contains("\"inf\":null"), "{json}");
    assert!(!json.contains("\"type\""), "{json}");
}

#[test]
fn document_json_natural_string() {
    let doc = teptris::Document::parse(b"v = 2.5\n").unwrap();
    let json = doc.to_json_natural_string().unwrap();
    assert!(json.contains("\"v\":2.5"), "{json}");
}
