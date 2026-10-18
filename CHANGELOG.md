# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
