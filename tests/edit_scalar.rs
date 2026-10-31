use teptris::edit_scalar;

const ERR_STATE: i32 = 7;
const ERR_ARG: i32 = 6;

#[test]
fn preserves_everything_but_the_value()
{
    let src = "# c\nversion = \"1.2.3\"  # t\n[serve]\nport = 8080\n";
    let got = edit_scalar(src, "serve.port", "9090").unwrap();
    assert_eq!(
        got,
        "# c\nversion = \"1.2.3\"  # t\n[serve]\nport = 9090\n"
    );
}

#[test]
fn multiline_and_inline_targets()
{
    let src = "m = \"\"\"\nline\n\"\"\"\nz = 1\n";
    assert_eq!(edit_scalar(src, "m", "\"n\"").unwrap(), "m = \"n\"\nz = 1\n");
    assert_eq!(
        edit_scalar("t = {a = 1}\n", "t", "{a = 2}").unwrap(),
        "t = {a = 2}\n"
    );
}

#[test]
fn crlf_is_preserved()
{
    let src = "# c\r\nport = 8080\r\n";
    assert_eq!(
        edit_scalar(src, "port", "9090").unwrap(),
        "# c\r\nport = 9090\r\n"
    );
}

#[test]
fn missing_key_is_state_error()
{
    match edit_scalar("a = 1\n", "b", "2") {
        Err(e) => assert_eq!(e.0, ERR_STATE),
        Ok(_) => panic!("expected ERR_STATE"),
    }
}

#[test]
fn malformed_replacement_is_arg_error()
{
    match edit_scalar("a = 1\n", "a", "@@@") {
        Err(e) => assert_eq!(e.0, ERR_ARG),
        Ok(_) => panic!("expected ERR_ARG"),
    }
}
