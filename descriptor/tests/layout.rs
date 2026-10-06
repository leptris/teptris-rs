//! Layout guards: the repr(C) records must hold the exact field
//! order/sizes the ABI doc promises. A mismatch here means the crate
//! no longer mirrors plan.h.

use teptris_descriptor::{PlanRow, PlanSpec};

#[test]
fn plan_row_layout() {
    assert_eq!(core::mem::size_of::<PlanRow>(), 16); // ptr + u8 + pad + u32
    let offsets = (
        core::mem::offset_of!(PlanRow, name),
        core::mem::offset_of!(PlanRow, kind),
        core::mem::offset_of!(PlanRow, sub),
    );
    assert_eq!(offsets, (0, 8, 12));
}

#[test]
fn plan_spec_layout() {
    assert_eq!(core::mem::size_of::<PlanSpec>(), 24); // 2 x u32 + 2 x ptr
    let offsets = (
        core::mem::offset_of!(PlanSpec, abi_version),
        core::mem::offset_of!(PlanSpec, plan_count),
        core::mem::offset_of!(PlanSpec, plans),
        core::mem::offset_of!(PlanSpec, plan_first_row),
    );
    assert_eq!(offsets, (0, 4, 8, 16));
}

#[test]
fn no_std_types_are_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<PlanRow>();
    assert_copy::<PlanSpec>();
}
