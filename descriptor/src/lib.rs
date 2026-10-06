//! Shared descriptor-plan ABI for the leptris engine family.
//!
//! One crate per engine (`leptris` XML, `yeptris` YAML, `teptris`
//! TOML/JSON) targets this record layout, so hosts build plan rows
//! once and every engine's compiled-descriptor walk speaks the same
//! shape. The types mirror `src/include/teptris/plan.h` in the
//! teptris engine — field order is the ABI; NEVER renumber or
//! reorder. Additive trailing fields only, guarded by an ABI version
//! bump.
//!
//! Layout is verified by the linking binding crate's tests
//! (`tests/descriptor_abi.rs`): a plan built through THESE types must
//! be accepted by the engine's `teptris_plan_build` and walked.
//!
//! This crate is `no_std`: types and constants only, plus the FFI
//! declarations behind the `link` feature.

#![no_std]
#![allow(non_camel_case_types, non_snake_case)]

use core::ffi::c_char;

/// Must equal `teptris_plan_abi_version()` reported by the engine.
pub const PLAN_ABI_VERSION: u32 = 1;

/// Row kinds. Append-only.
pub type PlanKind = u8;
pub const PLAN_KIND_SCALAR: PlanKind = 1;
pub const PLAN_KIND_COLLECTION: PlanKind = 2;
pub const PLAN_KIND_NESTED: PlanKind = 3;
pub const PLAN_KIND_RAW: PlanKind = 4;

/// One row of a flattened plan tree: a named binding of one kind,
/// NESTED rows pointing at a sub-plan index.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PlanRow {
    /// Table key this row binds (NUL-terminated; copied by
    /// `teptris_plan_build`).
    pub name: *const c_char,
    /// `PlanKind`.
    pub kind: u8,
    /// Sub-plan index (NESTED only; 0 otherwise).
    pub sub: u32,
}

/// Flattened plan tree: plan p owns rows
/// `[first_row[p], first_row[p + 1])`. The engine deep-copies; the
/// caller's memory is transient.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PlanSpec {
    pub abi_version: u32,
    pub plan_count: u32,
    pub plans: *const PlanRow,
    pub plan_first_row: *const u32,
}

/// Result node kinds (the walk output).
pub type PlanResultKind = u8;
pub const PLAN_RESULT_MISSING: PlanResultKind = 0;
pub const PLAN_RESULT_SCALAR: PlanResultKind = 1;
pub const PLAN_RESULT_ARRAY: PlanResultKind = 2;
pub const PLAN_RESULT_TABLE: PlanResultKind = 3;
pub const PLAN_RESULT_RAW: PlanResultKind = 4;

#[cfg(feature = "link")]
mod link {
    use core::ffi::{c_char, c_int, c_void};

    use super::{PlanRow, PlanSpec};

    /// Opaque handles (defined by the engine, not this crate).
    #[repr(C)]
    pub struct teptris_plan {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct teptris_plan_result {
        _private: [u8; 0],
    }
    pub type teptris_status = c_int;

    extern "C" {
        pub fn teptris_plan_build(
            spec: *const PlanSpec,
            status: *mut teptris_status,
        ) -> *mut teptris_plan;
        pub fn teptris_plan_free(plan: *mut teptris_plan);
        pub fn teptris_plan_walk(
            plan: *const teptris_plan,
            node: *const c_void,
            status: *mut teptris_status,
        ) -> *mut teptris_plan_result;
        pub fn teptris_plan_result_free(result: *mut teptris_plan_result);
        pub fn teptris_plan_row_count(plan: *const teptris_plan, plan_idx: u32) -> u32;
        pub fn teptris_plan_row_name_at(
            plan: *const teptris_plan,
            plan_idx: u32,
            row: u32,
        ) -> *const c_char;
        #[allow(dead_code)]
        pub fn teptris_plan_abi_version() -> u32;
    }
}

#[cfg(feature = "link")]
pub use link::*;
