# build-toolchain Specification

## Purpose
This capability governs the crate's minimum supported Rust version (MSRV) and toolchain configuration.

## Requirements
### Requirement: Minimum supported Rust version

The crate SHALL declare a minimum supported Rust version (MSRV) via `rust-version`
in `Cargo.toml`, and that value SHALL be at least the highest Rust version required
by any crate in the resolved dependency set across all features. The declared MSRV
SHALL be Rust 1.95, which satisfies the floor imposed by the `recall-tantivy`
dependency stack (notably `time >= 0.3.47`, which requires Rust 1.88). The project
SHALL build on the latest stable toolchain, and `rust-toolchain.toml` SHALL pin the
`stable` channel rather than a fixed version.

#### Scenario: Manifest declares the MSRV

- **WHEN** `Cargo.toml` is inspected
- **THEN** its `rust-version` field is `1.95` (or higher)
- **AND** the value is greater than or equal to the Rust version required by every dependency in `Cargo.lock` across all features, including the `recall-tantivy` feature's `time` dependency

#### Scenario: Toolchain tracks stable

- **WHEN** `rust-toolchain.toml` is inspected
- **THEN** its `channel` is `stable` (not a pinned version), so the project builds on the current stable toolchain that satisfies the declared MSRV
