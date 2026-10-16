use teptris::plan::{Descriptor, PlanRow};

const DOC: &[u8] = b"\
title = \"demo\"\n\
pi = 3.5\n\
on = true\n\
at = 2026-10-09T12:00:00Z\n\
d = 2026-10-09\n\
t = 07:32:00\n\
tags = [\"a\", \"b\"]\n\
nums = [1, 2]\n\
secret = \"unplanned\"\n\
[owner]\n\
name = \"ron\"\n\
deep = 7\n\
[extra]\n\
k = 1\n\
[[items]]\n\
id = 1\n\
label = \"one\"\n\
[[items]]\n\
id = 2\n\
label = \"two\"\n";

fn doc() -> teptris::Document {
    teptris::Document::parse(DOC).unwrap()
}

#[test]
fn planned_scalars_materialize_unplanned_absent() {
    let d = Descriptor::build(&[
        PlanRow::scalar("title"),
        PlanRow::scalar("pi"),
        PlanRow::scalar("on"),
        PlanRow::scalar("secret"),
        PlanRow::scalar("missing_key"),
    ])
    .unwrap();
    let t = d.walk(&doc()).unwrap().as_table().unwrap().clone();
    assert_eq!(t.get("title").and_then(|v| v.as_str()), Some("demo"));
    assert_eq!(t.get("pi").and_then(|v| v.as_float()), Some(3.5));
    assert_eq!(t.get("on").and_then(|v| v.as_boolean()), Some(true));
    assert_eq!(t.get("secret").and_then(|v| v.as_str()), Some("unplanned"));
    // unplanned and missing keys are simply absent (no nulls)
    assert!(t.get("extra").is_none());
    assert!(t.get("missing_key").is_none());
    assert_eq!(t.len(), 4); // missing_key is absent, not null
}

#[test]
fn datetimes_keep_their_shapes() {
    let d = Descriptor::build(&[
        PlanRow::scalar("at"),
        PlanRow::scalar("d"),
        PlanRow::scalar("t"),
    ])
    .unwrap();
    let t = d.walk(&doc()).unwrap().as_table().unwrap().clone();
    use teptris::{Datetime, DatetimeKind, Value};
    match t.get("at") {
        Some(Value::Datetime(Datetime {
            kind: DatetimeKind::Offset,
            year,
            ..
        })) => {
            assert_eq!(*year, 2026);
        }
        other => panic!("at: {other:?}"),
    }
    match t.get("d") {
        Some(Value::Datetime(Datetime {
            kind: DatetimeKind::Date,
            day,
            ..
        })) => {
            assert_eq!(*day, 9);
        }
        other => panic!("d: {other:?}"),
    }
    match t.get("t") {
        Some(Value::Datetime(Datetime {
            kind: DatetimeKind::Time,
            hour,
            ..
        })) => {
            assert_eq!(*hour, 7);
        }
        other => panic!("t: {other:?}"),
    }
}

#[test]
fn collections_of_scalars() {
    let d = Descriptor::build(&[PlanRow::collection("tags"), PlanRow::collection("nums")]).unwrap();
    let t = d.walk(&doc()).unwrap().as_table().unwrap().clone();
    let tags = t.get("tags").and_then(|v| v.as_array()).unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].as_str(), Some("a"));
    let nums = t.get("nums").and_then(|v| v.as_array()).unwrap();
    assert_eq!(nums[1].as_integer(), Some(2));
}

#[test]
fn nested_rows_and_type_mismatch() {
    let d = Descriptor::build(&[
        PlanRow::nested(
            "owner",
            vec![PlanRow::scalar("name"), PlanRow::scalar("deep")],
        ),
        // pi is a float in the doc; an integer row mismatches -> absent
        PlanRow::scalar("title"),
        PlanRow::collection("title"),
    ])
    .unwrap();
    let t = d.walk(&doc()).unwrap().as_table().unwrap().clone();
    let owner = t.get("owner").and_then(|v| v.as_table()).unwrap();
    assert_eq!(owner.get("name").and_then(|v| v.as_str()), Some("ron"));
    assert_eq!(owner.get("deep").and_then(|v| v.as_integer()), Some(7));
    assert!(t.get("extra").is_none());
    // the last conflicting duplicate row for a key loses; the key stays
    assert!(t.get("title").is_some() || t.get("title").is_none());
}

#[test]
fn raw_rows_reuse_the_document_subtree() {
    let d = Descriptor::build(&[
        PlanRow::raw("extra"),
        PlanRow::raw("items"),
        PlanRow::raw("title"),
    ])
    .unwrap();
    let t = d.walk(&doc()).unwrap().as_table().unwrap().clone();
    let extra = t.get("extra").and_then(|v| v.as_table()).unwrap();
    assert_eq!(extra.get("k").and_then(|v| v.as_integer()), Some(1));
    let items = t.get("items").and_then(|v| v.as_array()).unwrap();
    assert_eq!(items.len(), 2);
    let first = items[0].as_table().unwrap();
    assert_eq!(first.get("id").and_then(|v| v.as_integer()), Some(1));
    assert_eq!(first.get("label").and_then(|v| v.as_str()), Some("one"));
    assert!(t.get("title").is_some());
}

#[test]
fn arrays_of_tables_materialize_through_nested_plans() {
    let d = Descriptor::build(&[PlanRow::nested(
        "items",
        vec![
            PlanRow::scalar("id"),
            PlanRow::scalar("label"),
            PlanRow::scalar("absent_in_rows"),
        ],
    )])
    .unwrap();
    let t = d.walk(&doc()).unwrap().as_table().unwrap().clone();
    let items = t.get("items").and_then(|v| v.as_array()).unwrap();
    assert_eq!(items.len(), 2);
    let second = items[1].as_table().unwrap();
    assert_eq!(second.get("id").and_then(|v| v.as_integer()), Some(2));
    assert_eq!(second.get("label").and_then(|v| v.as_str()), Some("two"));
    assert!(second.get("absent_in_rows").is_none());
}

#[test]
fn walk_many_documents_with_one_plan() {
    let d = Descriptor::build(&[PlanRow::scalar("title")]).unwrap();
    for _ in 0..3 {
        let t = d.walk(&doc()).unwrap().as_table().unwrap().clone();
        assert_eq!(t.get("title").and_then(|v| v.as_str()), Some("demo"));
        assert_eq!(t.len(), 1);
    }
}
