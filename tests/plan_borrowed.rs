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

#[test]
fn borrowed_aot_tables_read_zero_copy() {
    let doc = teptris::Document::parse(
        b"[[items]]\nname = \"one\"\nweight = 1\n[[items]]\nname = \"two\"\nweight = 2\n",
    )
    .unwrap();
    let d = plan();
    let r = d.walk_borrowed(&doc).unwrap();
    let e0 = r.aot_table("items", 0).unwrap();
    assert_eq!(e0.get_str("name"), Some("one"));
    assert_eq!(e0.get_int("weight"), Some(1));
    let e1 = r.aot_table("items", 1).unwrap();
    assert_eq!(e1.get_str("name"), Some("two"));
    assert_eq!(e1.get_int("weight"), Some(2));
    // out-of-range and type mismatches are None, never a panic
    assert!(r.aot_table("items", 2).is_none());
    assert!(r.aot_table("meta", 0).is_none());
}

#[test]
fn borrowed_nested_chain_reaches_everywhere_owned_goes() {
    let doc = teptris::Document::parse(
        b"[meta]\nauthor = \"ron\"\n[meta.build]\nflags = \"release\"\n[[meta.checks]]\nname = \"fmt\"\n",
    )
    .unwrap();
    let d = Descriptor::build(&[PlanRow::nested(
        "meta",
        vec![
            PlanRow::scalar("author"),
            PlanRow::nested("build", vec![PlanRow::scalar("flags")]),
            PlanRow::nested("checks", vec![PlanRow::scalar("name")]),
        ],
    )])
    .unwrap();
    let r = d.walk_borrowed(&doc).unwrap();
    let meta = r.table("meta").unwrap();
    assert_eq!(meta.get_str("author"), Some("ron"));
    let build = meta.table("build").unwrap();
    assert_eq!(build.get_str("flags"), Some("release"));
    let checks = meta.aot_table("checks", 0).unwrap();
    assert_eq!(checks.get_str("name"), Some("fmt"));
}
