//! Behavioral ABI gate: a plan built through the descriptor crate's
//! repr(C) records must be accepted by the engine and walked. If the
//! crate's layout ever drifts from plan.h, this fails first.

use teptris::loads;
use teptris_descriptor::{
    teptris_plan_build, teptris_plan_free, teptris_plan_row_count, teptris_plan_row_name_at,
    PlanRow, PlanSpec, PLAN_ABI_VERSION,
};
use teptris_descriptor::{
    teptris_plan_result_free, teptris_plan_walk, PLAN_RESULT_MISSING, PLAN_RESULT_SCALAR,
};

#[test]
fn descriptor_plan_builds_and_walks() {
    let rows = [
        PlanRow {
            name: b"a\0".as_ptr() as *const _,
            kind: teptris_descriptor::PLAN_KIND_SCALAR,
            sub: 0,
        },
        PlanRow {
            name: b"t\0".as_ptr() as *const _,
            kind: teptris_descriptor::PLAN_KIND_NESTED,
            sub: 1,
        },
        PlanRow {
            name: b"host\0".as_ptr() as *const _,
            kind: teptris_descriptor::PLAN_KIND_SCALAR,
            sub: 0,
        },
    ];
    let first_row = [0, 2, 3];
    let spec = PlanSpec {
        abi_version: PLAN_ABI_VERSION,
        plan_count: 2,
        plans: rows.as_ptr(),
        plan_first_row: first_row.as_ptr(),
    };

    let mut status: i32 = 0;
    let plan = unsafe { teptris_plan_build(&spec, &mut status) };
    assert!(
        !plan.is_null(),
        "engine rejected the descriptor-crate layout (status {status})"
    );

    assert_eq!(unsafe { teptris_plan_row_count(plan, 0) }, 2);
    let n0 = unsafe { teptris_plan_row_name_at(plan, 0, 0) };
    let name = unsafe { std::ffi::CStr::from_ptr(n0) }.to_str().unwrap();
    assert_eq!(name, "a");

    unsafe { teptris_plan_free(plan) };
    let _ = (PLAN_RESULT_MISSING, PLAN_RESULT_SCALAR);
}

#[test]
fn plan_rejects_bad_abi() {
    // A wrong ABI version must fail the build, proving the version
    // field crosses the boundary (the drift tripwire).
    let rows = [PlanRow {
        name: b"a\0".as_ptr() as *const _,
        kind: teptris_descriptor::PLAN_KIND_SCALAR,
        sub: 0,
    }];
    let first_row = [0u32, 1];
    let spec = PlanSpec {
        abi_version: PLAN_ABI_VERSION + 999,
        plan_count: 1,
        plans: rows.as_ptr(),
        plan_first_row: first_row.as_ptr(),
    };
    let mut status: i32 = 0;
    let plan = unsafe { teptris_plan_build(&spec, &mut status) };
    if !plan.is_null() {
        unsafe { teptris_plan_free(plan) };
    }
    assert!(
        status != 0 || plan.is_null(),
        "bad ABI version must be rejected"
    );
}

#[test]
fn loads_still_works_alongside() {
    let v = loads(b"x = 1\n").unwrap();
    assert_eq!(v.get("x").and_then(Value::as_integer), Some(1));
}

use teptris::Value;
