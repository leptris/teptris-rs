//! Schema validation against the JSON Schema 2020-12 practical
//! subset ([engine `SCHEMAS.md`](https://github.com/leptris/teptris)).
//!
//! Compile once, validate many: [`Schema`] is an immutable,
//! arena-backed handle. Validation returns every violation as a typed
//! [`Violation`]; an empty vec means the document is valid. A document
//! that does not parse is [`Error`](`crate::Error`) with the parse
//! status, never a violation.
//!
//! ```no_run
//! use teptris::schema::Schema;
//!
//! let schema = Schema::compile(r#"{"properties":{"port":{"type":"integer"}}}"#).unwrap();
//! let violations = schema.validate(b"port = \"x\"\n").unwrap();
//! assert_eq!(violations[0].path, "port");
//! ```

use std::fmt;
use std::os::raw::{c_char, c_int};

use crate::ffi;
use crate::{libc_free, Document, Error};

/// The compiled-schema handle. Cheap to clone-free share via `&`.
#[derive(Debug)]
pub struct Schema {
    ptr: *mut ffi::teptris_schema,
}

// The handle is immutable after compile (the compiled graph is
// read-only; every validate call owns its scratch context), so a
// shared Schema is usable from many threads - the same
// distinct-object thread contract as Document.
unsafe impl Send for Schema {}
unsafe impl Sync for Schema {}

/// A compile failure: carries the engine's diag JSON
/// (`{"error": "..."}`) verbatim — unsupported constructs are rejected
/// with a named keyword, never silently ignored.
#[derive(Clone, Debug)]
pub struct SchemaCompileError {
    pub diag: String,
}

impl fmt::Display for SchemaCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "schema compile failed: {}", self.diag)
    }
}

impl std::error::Error for SchemaCompileError {}

/// One validation failure: where in the document (`path`, the
/// materialized TOML path), where in the schema (`schema_path`), and
/// why (`message`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    pub path: String,
    pub schema_path: String,
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl Schema {
    /// Compile a JSON Schema. Unsupported keywords, remote `$ref`s and
    /// malformed JSON are rejected here with
    /// [`SchemaCompileError::diag`].
    pub fn compile(schema_json: &str) -> Result<Schema, SchemaCompileError> {
        let mut ptr: *mut ffi::teptris_schema = std::ptr::null_mut();
        let mut diag: *mut c_char = std::ptr::null_mut();
        let mut diag_len: usize = 0;
        let st = unsafe {
            ffi::teptris_schema_compile(
                schema_json.as_ptr() as *const c_char,
                schema_json.len(),
                &mut ptr,
                &mut diag,
                &mut diag_len,
            )
        };
        if st != ffi::TEPTRIS_OK {
            let text = if diag.is_null() {
                "invalid schema".to_string()
            } else {
                let d = unsafe { std::slice::from_raw_parts(diag as *const u8, diag_len) };
                let owned = String::from_utf8_lossy(d).into_owned();
                unsafe { libc_free(diag as *mut _) };
                owned
            };
            return Err(SchemaCompileError { diag: text });
        }
        Ok(Schema { ptr })
    }

    /// Validate raw TOML bytes; an empty vec = valid.
    pub fn validate(&self, toml: &[u8]) -> Result<Vec<Violation>, Error> {
        let mut buf: *mut c_char = std::ptr::null_mut();
        let mut out_len: usize = 0;
        let st = unsafe {
            ffi::teptris_schema_validate(
                self.ptr,
                toml.as_ptr() as *const c_char,
                toml.len(),
                &mut buf,
                &mut out_len,
            )
        };
        check_status(st)?;
        decode_violations(buf, out_len)
    }

    /// Validate a parsed document (the `validate_doc` entrypoint:
    /// reuse an existing parse instead of parsing twice).
    pub fn validate_doc(&self, doc: &Document) -> Result<Vec<Violation>, Error> {
        let mut buf: *mut c_char = std::ptr::null_mut();
        let mut out_len: usize = 0;
        let st = unsafe {
            ffi::teptris_schema_validate_doc(self.ptr, doc.as_raw_ptr(), &mut buf, &mut out_len)
        };
        check_status(st)?;
        decode_violations(buf, out_len)
    }

    /// The boolean form: true when the document has no violations
    /// (and parses).
    pub fn is_valid(&self, toml: &str) -> bool {
        self.validate(toml.as_bytes())
            .map(|v| v.is_empty())
            .unwrap_or(false)
    }
}

impl Drop for Schema {
    fn drop(&mut self) {
        unsafe { ffi::teptris_schema_free(self.ptr) };
    }
}

fn check_status(st: ffi::TeptrisStatus) -> Result<(), Error> {
    if st == ffi::TEPTRIS_OK {
        return Ok(());
    }
    Err(Error(st as c_int))
}

/// The engine hands back one malloc'd NUL-terminated JSON array of
/// `{"path","schemaPath","message"}` objects; free it with libc and
/// decode the flat string fields.
fn decode_violations(buf: *mut c_char, len: usize) -> Result<Vec<Violation>, Error> {
    if buf.is_null() {
        return Err(Error(ffi::TEPTRIS_ERR_STATE as c_int));
    }
    let raw = unsafe { std::slice::from_raw_parts(buf as *const u8, len) };
    let text = String::from_utf8_lossy(raw).into_owned();
    unsafe { libc_free(buf as *mut _) };
    Ok(parse_violations(&text))
}

/// A minimal reader for the engine's flat error envelope: an array of
/// objects whose three keys map to JSON strings (escapes: \" \\ \/ \b
/// \f \n \r \t \uXXXX).
fn parse_violations(text: &str) -> Vec<Violation> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut i = 0usize;
    while i < b.len() && b[i] != b'[' {
        i += 1;
    }
    while i < b.len() {
        if b[i] == b'{' {
            let mut path = String::new();
            let mut schema_path = String::new();
            let mut message = String::new();
            i += 1;
            while i < b.len() && b[i] != b'}' {
                // object key
                let (key, ni) = match read_string(b, i) {
                    Some(kv) => kv,
                    None => return out,
                };
                i = ni;
                while i < b.len() && (b[i] == b':' || b[i] == b' ') {
                    i += 1;
                }
                let (val, ni) = match read_string(b, i) {
                    Some(kv) => kv,
                    None => return out,
                };
                i = ni;
                match key.as_str() {
                    "path" => path = val,
                    "schemaPath" => schema_path = val,
                    "message" => message = val,
                    _ => {}
                }
                while i < b.len() && (b[i] == b',' || b[i] == b' ') {
                    i += 1;
                }
            }
            if !message.is_empty() {
                out.push(Violation {
                    path,
                    schema_path,
                    message,
                });
            }
        }
        i += 1;
    }
    out
}

fn read_string(b: &[u8], start: usize) -> Option<(String, usize)> {
    let mut i = start;
    while i < b.len() && b[i] != b'"' {
        i += 1;
    }
    if i >= b.len() {
        return None;
    }
    i += 1;
    let mut out = Vec::new();
    while i < b.len() {
        match b[i] {
            b'"' => return Some((String::from_utf8_lossy(&out).into_owned(), i + 1)),
            b'\\' => {
                i += 1;
                if i >= b.len() {
                    return None;
                }
                match b[i] {
                    b'"' => out.push(b'"'),
                    b'\\' => out.push(b'\\'),
                    b'/' => out.push(b'/'),
                    b'b' => out.push(0x08),
                    b'f' => out.push(0x0C),
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'u' => {
                        if i + 4 >= b.len() {
                            return None;
                        }
                        let mut cp: u32 = 0;
                        for k in 1..=4 {
                            let d = (b[i + k] as char).to_digit(16)?;
                            cp = cp * 16 + d;
                        }
                        i += 4;
                        // the envelope escapes only quotes/backslashes and
                        // control chars (as \uXXXX); surrogate pairs arrive
                        // as raw UTF-8, so a lone \u here is BMP-only
                        if (0xD800..0xE000).contains(&cp) {
                            out.extend_from_slice("\u{FFFD}".as_bytes());
                        } else {
                            let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                            let mut tmp = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                        }
                    }
                    other => out.push(other),
                }
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    None
}
