# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.16] - 2026-10-31

### Added

- edit_scalar - the format-preserving editor surfaces in Rust (multiline + inline targets splice; CRLF kept; rides engine v0.3.16)

## [0.3.15] - 2026-10-31

### Fixed

- The engine pin rides v0.3.15 (HH:MM at true EOF overread one byte - the nightly fuzz find)

## [0.3.14] - 2026-10-31

### Changed

- The engine pin rides v0.3.14 (the format-preserving edit + the edit fuzz lane)

### Added

- The borrowed walk - PlanResult typed getters over the engine result view


## [0.3.12] - 2026-10-31

### Changed

- This wave release rides engine v0.3.12 per the all-runtimes law
  (CRLF multi-line normalization + the dotted-key depth ceiling - the
  rust surface's own code is unchanged; the crate links the engine
  from source, so consumers pick the fixes up when they rebuild)

## [0.3.11] - 2026-10-30

### Fixed

- The rust toolchain is pinned (the fmt gate passed PR #41 and failed
  main when stable floated between runs)
- The empty-key writer contract is pinned at the rust surface (engine
  v0.3.10 emits quoted empty keys; the rust builder path was never
  broken)
- This wave release rides engine v0.3.11 per the all-runtimes law
  (v0.3.11 repairs the v0.3.10 array-marker regression in the op-stream
  writer; the rust builder path was unaffected)


## [0.3.9] - 2026-10-30

### Changed

- The crate version now matches the engine version it is validated
  against (owner ruling, 2026-10-30): teptris-rs 0.3.9 rides engine
  v0.3.9. The old 0.1.x line is retired; every future crate release
  carries the engine's number per the family all-runtimes law.

## [0.1.7] - 2026-10-29

### Added

- The plan tier - plan-direct extraction vs the derive path: the
  descriptor's crossover measured (1.52x over serde derive at 2/8
  planned fields, parity at 4/8, 0.69x at full breadth - the
  owned-Value walk is a selective-extraction tool; the parse floor
  sits 2.59x under derive), plus the multi-pair inline-table
  roundtrip test pinning the engine-writer contract (teptris#150) at
  the rust surface
- The all-runtimes release law: this wave release rides engine
  v0.3.9 (CI builds against its tag; the crate links the engine from
  source, so consumers pick the fix up when they rebuild)

## [0.1.6] - 2026-10-22

### Added

- Serde support - from_str/to_string into your structs, Value/Table/Datetime derive-compatible (B2)

### Other

- The plan-fusion ceiling - derive minus Value is 11.6% (0.26 ms of 2.29)

- The serde tier - derive-vs-derive vs the toml crate (B2's cell)

- Ci badge (X4)

- Cargo fmt on the extended tier

- The roundtrip cell vs the toml crate + toml_edit

- Checked-in TOML corpus stays LF (windows autocrlf would shift multiline strings in the differential)

- Cargo fmt

- Binding-level differential vs the toml crate (B5)

- The mission - CLAUDE.md + README banner (BEST-EVER bar)

## [0.1.5] - 2026-10-18

### Fixed

- CI builds against engine tag v0.3.4 — `teptris_plan_build` no longer
  writes debug output to stderr (a DBG fprintf shipped in the v0.3.2
  depth gate spammed one line per plan row in every host).

## [0.1.4] - 2026-10-18

### Fixed

- CI builds against engine tag v0.3.3 (was: engine `main`) — brings the
  plan depth cap (engine v0.3.2) and the dom clone NULL-memcpy UB guards
  (engine v0.3.3) into every CI build; the pin moves with the release
  per the family versioning law.

## [0.1.3] - 2026-10-17

### Added

- Dump_json_natural — the natural-JSON emit for hosts
- Descriptor — planned-key materialization; sibling nested rows no
  longer share a sub-plan
- The descriptor-plan API and the JSON emit surface in the readme

### Fixed

- Drop the author byline — commit.author is an object
- Engine version assert must not hard-code the minor — CI builds
  against the engine main line
- Only feat/fix commits trigger release PRs
- actions/checkout v7 — clears the Node.js 20 deprecation annotation

### Other

- The rust tier — teptris-rs vs the toml crate
- Run on github.token — the org PAT 404s in this repo
- The action's command is release-pr, not update
- Decouple release-PR opening from registry auth
