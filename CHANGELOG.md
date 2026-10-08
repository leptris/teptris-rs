# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Engine version assert must not hard-code the minor - CI builds against engine main (0.3.0 today) by @[object]

### Other

- Run on github.token — the org PAT 404s in this repo by @[object]
- The action's command is release-pr, not update by @[object]
- Decouple release-PR opening from registry auth by @[object]
