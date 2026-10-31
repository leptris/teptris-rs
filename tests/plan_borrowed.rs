use teptris::{BorrowedKind, Descriptor, PlanRow};

fn plan() -> Descriptor {
    Descriptor::build(&[
        PlanRow::scalar("title"),
        PlanRow::scalar("count"),
        PlanRow::scalar("ratio"),
        PlanRow::scalar("on"),
        PlanRow::collection("tags"),
        PlanRow::nested("meta", vec![PlanRow::scalar("author")]),
        PlanRow::nested(
            "items",
            vec![PlanRow::scalar("name"), PlanRow::scalar("weight")],
        ),
    ])
    .unwrap()
}

#[test]
fn borrowed_scalars_read_zero_copy() {
    let doc = teptris::Document::parse(b"title = \"demo\"\ncount = 7\nratio = 0.25\non = true\n")
        .unwrap();
    let d = plan();
    let r = d.walk_borrowed(&doc).unwrap();
    assert_eq!(r.get_str("title"), Some("demo"));
    assert_eq!(r.get_int("count"), Some(7));
    assert_eq!(r.get_float("ratio"), Some(0.25));
    assert_eq!(r.get_bool("on"), Some(true));
}

#[test]
fn borrowed_arrays_and_nested() {
    let doc = teptris::Document::parse(
        b"tags = [\"a\", \"b\"]\n[meta]\nauthor = \"ron\"\n[[items]]\nname = \"one\"\nweight = 1\n[[items]]\nname = \"two\"\nweight = 2\n",
    )
    .unwrap();
    let d = plan();
    let r = d.walk_borrowed(&doc).unwrap();
    assert_eq!(r.array_len("tags"), Some(2));
    assert_eq!(r.array_str("tags", 0), Some("a"));
    assert_eq!(r.array_str("tags", 1), Some("b"));
    let meta = r.table("meta").unwrap();
    assert_eq!(meta.get_str("author"), Some("ron"));
    assert_eq!(r.aot_len("items"), Some(2));
    assert_eq!(
        r.aot_entry("items", 1).unwrap().as_table().unwrap().len(),
        2
    );
}

#[test]
fn missing_and_type_mismatches() {
    let doc = teptris::Document::parse(b"title = \"x\"\n").unwrap();
    let d = plan();
    let r = d.walk_borrowed(&doc).unwrap();
    assert_eq!(r.kind("count"), BorrowedKind::Missing);
    assert_eq!(r.get_int("count"), None);
    assert_eq!(r.get_str("count"), None); // absent row: no value
    assert!(r.table("meta").is_none());
}

#[test]
fn borrowed_matches_owned_walk() {
    let src = b"title = \"demo\"\ncount = 7\nratio = 0.25\non = true\n";
    let doc = teptris::Document::parse(src).unwrap();
    let d = plan();
    let owned = d.walk(&doc).unwrap();
    let borrowed = d.walk_borrowed(&doc).unwrap();
    assert_eq!(
        borrowed.get_str("title"),
        owned.as_table().unwrap().get("title").unwrap().as_str()
    );
    assert_eq!(
        borrowed.get_int("count"),
        owned.as_table().unwrap().get("count").unwrap().as_integer()
    );
}
