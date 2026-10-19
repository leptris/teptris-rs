# CLAUDE.md — teptris-rs

## Mission

Be the best TOML parser and builder ever made — in C, Ruby, Rust, and
JS — measured, not claimed. This binding's bar, and the family
ledger/plan, live in the engine's
[BEST-EVER.md](https://github.com/leptris/teptris/blob/main/BEST-EVER.md).
"At parity" or "unmeasured" is an open gap.

## Operating laws (this binding)

- Version is 3-part semver (current: 0.1.5); releases are
  release-plz-driven — a `fix:`/`feat:` commit seeds the proposal, the
  proposal PR's changelog is deduped ON the proposal branch (cliff
  regenerates from commits and duplicates hand-written Unreleased
  content), then merged; the release.yml phase publishes to crates.io
  via Trusted Publishing.
- The engine pin is `ref:` in ci.yml (engine checkout at `path:
  engine`) — **a version-moving commit moves the pin**. The published
  crate links the consumer's libteptris (TEPTRIS_LIB_PATH / `-lteptris`);
  CI builds the pinned engine source.
- Every release is gated by a published-crate smoke: `cargo add` from
  crates.io in a throwaway project, build against the current engine
  shared lib (TEPTRIS_LIB_PATH + DYLD_LIBRARY_PATH on macOS), parse.
- serde support is an OPEN ledger item and an owner decision (adds a
  dependency) — do not add it unilaterally.
- All changes through PRs (rebase-merge); cargo test gates the merge.
