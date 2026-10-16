//! The descriptor-plan API (the rust twin of teptris-ruby's
//! `Teptris::Descriptor` and teptris-py's `Descriptor`): compile a
//! plan once, then materialize only the planned keys of a document
//! in ONE engine walk. Unplanned keys are never materialized; keys
//! absent from the document (or type-mismatched rows) are absent
//! from the output (rust has no null to stand in for ruby's nil).

use std::ffi::CString;

use crate::ffi;
use crate::{Datetime, DatetimeKind, Document, Error, Table, Value};

/// Result-row kinds (walk output), mirroring plan.h's anonymous enum.
mod result_kind {
    pub const MISSING: u8 = 0;
    pub const SCALAR: u8 = 1;
    pub const ARRAY: u8 = 2;
    pub const TABLE: u8 = 3;
    pub const RAW: u8 = 4;
}

#[derive(Debug, Clone)]
pub enum PlanRowKind {
    /// A scalar value.
    Scalar,
    /// An array (of scalars, or of tables for nested rows).
    Collection,
    /// The untouched document subtree.
    Raw,
    /// A sub-table walked against a nested plan.
    Nested(Vec<PlanRow>),
}

#[derive(Debug, Clone)]
pub struct PlanRow {
    name: String,
    kind: PlanRowKind,
}

impl PlanRow {
    pub fn scalar(name: impl Into<String>) -> Self {
        PlanRow {
            name: name.into(),
            kind: PlanRowKind::Scalar,
        }
    }
    pub fn collection(name: impl Into<String>) -> Self {
        PlanRow {
            name: name.into(),
            kind: PlanRowKind::Collection,
        }
    }
    pub fn raw(name: impl Into<String>) -> Self {
        PlanRow {
            name: name.into(),
            kind: PlanRowKind::Raw,
        }
    }
    pub fn nested(name: impl Into<String>, rows: Vec<PlanRow>) -> Self {
        PlanRow {
            name: name.into(),
            kind: PlanRowKind::Nested(rows),
        }
    }
}

/// A compiled plan. Build once, walk any number of documents.
pub struct Descriptor {
    plan: *mut ffi::teptris_plan,
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        if !self.plan.is_null() {
            unsafe { ffi::teptris_plan_free(self.plan) }
        }
    }
}

/// A planned row is one of: scalar, collection, raw subtree, or a
/// nested table walked against `rows`.
impl Descriptor {
    pub fn build(rows: &[PlanRow]) -> Result<Descriptor, Error> {
        let mut flat: Vec<ffi::teptris_plan_row> = Vec::new();
        let mut first: Vec<u32> = vec![0];
        let mut names: Vec<CString> = Vec::new();
        layout(rows, &mut flat, &mut first, &mut names);
        first.push(flat.len() as u32);

        let spec = ffi::teptris_plan_spec {
            abi_version: ffi::TEPTRIS_PLAN_ABI_VERSION,
            plan_count: first.len() as u32 - 1,
            plans: flat.as_ptr(),
            plan_first_row: first.as_ptr(),
        };
        let mut status = ffi::TEPTRIS_OK;
        let plan = unsafe { ffi::teptris_plan_build(&spec, &mut status) };
        check(status)?;
        if plan.is_null() {
            return Err(Error(ffi::TEPTRIS_ERR_STATE));
        }
        Ok(Descriptor { plan })
    }

    /// Materialize the planned keys of `doc`'s root table.
    pub fn walk(&self, doc: &Document) -> Result<Value, Error> {
        unsafe {
            let root = ffi::teptris_document_root(doc.ptr);
            let mut status = ffi::TEPTRIS_OK;
            let res = ffi::teptris_plan_walk(self.plan, root, &mut status);
            check(status)?;
            if res.is_null() {
                return Err(Error(ffi::TEPTRIS_ERR_STATE));
            }
            let out = asm(self.plan, res, 0);
            ffi::teptris_plan_result_free(res);
            Ok(Value::Table(out))
        }
    }
}

/// Depth-first plan layout: plans' row ranges stay contiguous, each
/// nested row points at its sub-plan's index.
fn layout(
    rows: &[PlanRow],
    flat: &mut Vec<ffi::teptris_plan_row>,
    first: &mut Vec<u32>,
    names: &mut Vec<CString>,
) -> u32 {
    // The C layout contract: plan p owns the contiguous row range
    // [first[p], first[p+1]). So a plan's OWN rows go first, and each
    // nested row's sub-plan is laid out after them — first[] grows in
    // plan order and the sub index is known at row-push time.
    let idx = first.len() as u32 - 1;
    let mut pending: Vec<(usize, &PlanRow)> = Vec::new();
    for r in rows {
        let cname = CString::new(r.name.as_str()).expect("plan row name");
        let ptr = cname.as_ptr();
        names.push(cname);
        match &r.kind {
            PlanRowKind::Scalar => flat.push(ffi::teptris_plan_row {
                name: ptr,
                kind: ffi::TEPTRIS_PLAN_SCALAR,
                sub: 0,
            }),
            PlanRowKind::Collection => flat.push(ffi::teptris_plan_row {
                name: ptr,
                kind: ffi::TEPTRIS_PLAN_COLLECTION,
                sub: 0,
            }),
            PlanRowKind::Raw => flat.push(ffi::teptris_plan_row {
                name: ptr,
                kind: ffi::TEPTRIS_PLAN_RAW,
                sub: 0,
            }),
            PlanRowKind::Nested(_) => {
                // sub is patched after the child block is laid out —
                // SIBLING nested rows must not capture the same index
                let slot = flat.len();
                flat.push(ffi::teptris_plan_row {
                    name: ptr,
                    kind: ffi::TEPTRIS_PLAN_NESTED,
                    sub: 0,
                });
                pending.push((slot, r));
            }
        }
    }
    for (slot, r) in pending {
        if let PlanRowKind::Nested(children) = &r.kind {
            first.push(flat.len() as u32);
            let got = layout(children, flat, first, names);
            flat[slot].sub = got;
        }
    }
    idx
}

fn check(st: i32) -> Result<(), Error> {
    if st == ffi::TEPTRIS_OK {
        Ok(())
    } else {
        Err(Error(st))
    }
}

/// Materialize plan `plan_idx`'s rows from a walked result.
fn asm(p: *const ffi::teptris_plan, res: *const ffi::teptris_plan_result, plan_idx: u32) -> Table {
    let n = unsafe { ffi::teptris_plan_row_count(p, plan_idx) };
    let mut entries: Vec<(String, Value)> = Vec::with_capacity(n as usize);
    for i in 0..n {
        if unsafe { ffi::teptris_plan_result_kind_at(res, i) } == result_kind::MISSING {
            continue;
        }
        let name = unsafe {
            let c = ffi::teptris_plan_row_name_at(p, plan_idx, i);
            std::ffi::CStr::from_ptr(c).to_string_lossy().into_owned()
        };
        let val = unsafe { asm_row(p, res, plan_idx, i) };
        entries.push((name, val));
    }
    Table::from_entries(entries)
}

unsafe fn asm_row(
    p: *const ffi::teptris_plan,
    res: *const ffi::teptris_plan_result,
    plan_idx: u32,
    i: u32,
) -> Value {
    match ffi::teptris_plan_result_kind_at(res, i) {
        result_kind::SCALAR => asm_scalar_value(res, i),
        result_kind::ARRAY => {
            let n2 = ffi::teptris_plan_result_array_len_at(res, i);
            let mut out = Vec::with_capacity(n2 as usize);
            for j in 0..n2 {
                match ffi::teptris_plan_result_array_kind_at(res, i, j) {
                    result_kind::SCALAR => out.push(asm_array_scalar_value(res, i, j)),
                    result_kind::TABLE => {
                        let sub = ffi::teptris_plan_result_array_entry_at(res, i, j);
                        if !sub.is_null() {
                            let sub_idx = ffi::teptris_plan_row_sub_at(p, plan_idx, i);
                            out.push(Value::Table(asm(p, sub, sub_idx)));
                            ffi::teptris_plan_result_view_free(sub);
                        }
                    }
                    _ => {}
                }
            }
            Value::Array(out)
        }
        result_kind::TABLE => {
            let sub = ffi::teptris_plan_result_row_view(res, i);
            if sub.is_null() {
                return Value::Table(Table::default());
            }
            let sub_idx = ffi::teptris_plan_row_sub_at(p, plan_idx, i);
            let out = Value::Table(asm(p, sub, sub_idx));
            ffi::teptris_plan_result_view_free(sub);
            out
        }
        result_kind::RAW => {
            let raw = ffi::teptris_plan_result_raw_at(res, i);
            if raw.is_null() {
                return Value::Table(Table::default());
            }
            match crate::node_to_value(raw) {
                Ok(v) => v,
                Err(_) => Value::Table(Table::default()),
            }
        }
        // MISSING: the key is absent from the document — rust has no
        // null, so the row is simply not represented.
        _ => Value::Table(Table::default()),
    }
}

unsafe fn datetime_from(res: *const ffi::teptris_plan_result, i: u32, kind: i32) -> Value {
    let mut d: ffi::teptris_datetime = std::mem::zeroed();
    if ffi::teptris_plan_result_datetime_at(res, i, &mut d) != ffi::TEPTRIS_OK {
        return Value::Table(Table::default());
    }
    let k = match kind {
        ffi::TEPTRIS_DATETIME_OFFSET => DatetimeKind::Offset,
        ffi::TEPTRIS_DATETIME_LOCAL => DatetimeKind::LocalDateTime,
        ffi::TEPTRIS_DATE_LOCAL => DatetimeKind::Date,
        _ => DatetimeKind::Time,
    };
    Value::Datetime(Datetime {
        kind: k,
        year: d.year,
        month: d.month,
        day: d.day,
        hour: d.hour,
        minute: d.minute,
        second: d.second,
        nanosecond: d.nanosecond,
        offset_seconds: if k == DatetimeKind::Offset {
            Some(d.offset_seconds)
        } else {
            None
        },
    })
}

unsafe fn asm_scalar_value(res: *const ffi::teptris_plan_result, i: u32) -> Value {
    match ffi::teptris_plan_result_value_kind_at(res, i) as i32 {
        ffi::TEPTRIS_STRING => {
            let mut v: ffi::teptris_view = std::mem::zeroed();
            if ffi::teptris_plan_result_string_at(res, i, &mut v) == ffi::TEPTRIS_OK {
                let bytes = std::slice::from_raw_parts(v.ptr as *const u8, v.len);
                Value::String(String::from_utf8_lossy(bytes).into_owned())
            } else {
                Value::Table(Table::default())
            }
        }
        ffi::TEPTRIS_INTEGER => {
            let mut out: i64 = 0;
            if ffi::teptris_plan_result_integer_at(res, i, &mut out) == ffi::TEPTRIS_OK {
                Value::Integer(out)
            } else {
                Value::Table(Table::default())
            }
        }
        ffi::TEPTRIS_FLOAT => {
            let mut out: f64 = 0.0;
            if ffi::teptris_plan_result_float_at(res, i, &mut out) == ffi::TEPTRIS_OK {
                Value::Float(out)
            } else {
                Value::Table(Table::default())
            }
        }
        ffi::TEPTRIS_BOOLEAN => {
            let mut out: bool = false;
            if ffi::teptris_plan_result_boolean_at(res, i, &mut out) == ffi::TEPTRIS_OK {
                Value::Boolean(out)
            } else {
                Value::Table(Table::default())
            }
        }
        ffi::TEPTRIS_DATETIME_OFFSET
        | ffi::TEPTRIS_DATETIME_LOCAL
        | ffi::TEPTRIS_DATE_LOCAL
        | ffi::TEPTRIS_TIME_LOCAL => datetime_from(
            res,
            i,
            ffi::teptris_plan_result_value_kind_at(res, i) as i32,
        ),
        _ => Value::Table(Table::default()),
    }
}

unsafe fn asm_array_scalar_value(res: *const ffi::teptris_plan_result, i: u32, j: u32) -> Value {
    match ffi::teptris_plan_result_array_value_kind_at(res, i, j) as i32 {
        ffi::TEPTRIS_STRING => {
            let mut v: ffi::teptris_view = std::mem::zeroed();
            if ffi::teptris_plan_result_array_string_at(res, i, j, &mut v) == ffi::TEPTRIS_OK {
                let bytes = std::slice::from_raw_parts(v.ptr as *const u8, v.len);
                Value::String(String::from_utf8_lossy(bytes).into_owned())
            } else {
                Value::Table(Table::default())
            }
        }
        ffi::TEPTRIS_INTEGER => {
            let mut out: i64 = 0;
            if ffi::teptris_plan_result_array_integer_at(res, i, j, &mut out) == ffi::TEPTRIS_OK {
                Value::Integer(out)
            } else {
                Value::Table(Table::default())
            }
        }
        ffi::TEPTRIS_FLOAT => {
            let mut out: f64 = 0.0;
            if ffi::teptris_plan_result_array_float_at(res, i, j, &mut out) == ffi::TEPTRIS_OK {
                Value::Float(out)
            } else {
                Value::Table(Table::default())
            }
        }
        ffi::TEPTRIS_BOOLEAN => {
            let mut out: bool = false;
            if ffi::teptris_plan_result_array_boolean_at(res, i, j, &mut out) == ffi::TEPTRIS_OK {
                Value::Boolean(out)
            } else {
                Value::Table(Table::default())
            }
        }
        ffi::TEPTRIS_DATETIME_OFFSET
        | ffi::TEPTRIS_DATETIME_LOCAL
        | ffi::TEPTRIS_DATE_LOCAL
        | ffi::TEPTRIS_TIME_LOCAL => {
            let mut d: ffi::teptris_datetime = std::mem::zeroed();
            if ffi::teptris_plan_result_array_datetime_at(res, i, j, &mut d) != ffi::TEPTRIS_OK {
                return Value::Table(Table::default());
            }
            let vk = ffi::teptris_plan_result_array_value_kind_at(res, i, j);
            let k = match vk as i32 {
                ffi::TEPTRIS_DATETIME_OFFSET => DatetimeKind::Offset,
                ffi::TEPTRIS_DATETIME_LOCAL => DatetimeKind::LocalDateTime,
                ffi::TEPTRIS_DATE_LOCAL => DatetimeKind::Date,
                _ => DatetimeKind::Time,
            };
            Value::Datetime(Datetime {
                kind: k,
                year: d.year,
                month: d.month,
                day: d.day,
                hour: d.hour,
                minute: d.minute,
                second: d.second,
                nanosecond: d.nanosecond,
                offset_seconds: if k == DatetimeKind::Offset {
                    Some(d.offset_seconds)
                } else {
                    None
                },
            })
        }
        _ => Value::Table(Table::default()),
    }
}
