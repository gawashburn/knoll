# knoll Repository Notes

## Project

`knoll` is a Rust command-line tool for reading and applying macOS display
configurations. It supports JSON and RON configuration groups, plus pipeline,
list, and daemon modes.

This project is macOS-specific. The real display backend links against
System and PrivateFrameworks and uses CoreGraphics APIs; keep Linux support
out of scope unless explicitly requested.

## Structure

- `src/main.rs` is the binary entry point.
- `src/lib.rs` exposes modules for integration tests.
- `src/knoll.rs` owns CLI parsing and command execution.
- `src/config.rs`, `src/valid_config.rs`, and `src/serde.rs` handle
  configuration data, validation, and JSON/RON serialization.
- `src/displays.rs` defines display abstractions shared by real and fake
  backends.
- `src/real_displays.rs` and `src/core_graphics.rs` interact with macOS.
- `src/fake_displays.rs` is the test backend.
- `tests/knoll_tests.rs` contains integration tests for CLI behavior.
- `examples/*.ron` are user-facing configuration examples.

## Development

- Rust edition is 2024; the minimum supported Rust version is 1.96.0.
- Prefer `nix develop` for local development so the shell uses the repository's
  pinned Rust toolchain and Nix inputs.
- The checked-in Rust toolchain is pinned in `rust-toolchain.toml`; keep
  `Cargo.toml`'s `rust-version` and the Nix flake toolchain in sync.
- Editors should use the repository-provided `rust-analyzer` from the pinned
  toolchain, preferably via `nix develop`, so rust-analyzer and Cargo support
  the same command-line options.
- Nix support is provided by `flake.nix` and `flake.lock`. The flake uses
  crane for Rust builds and fenix for stable and nightly Rust toolchains.
- Do not hand-edit `Cargo.lock` except as the direct result of dependency
  changes.
- Keep changes narrow; avoid broad cleanup when touching CoreGraphics or CLI
  behavior.

## Verification

- Run tests through the Nix development shell:

  ```bash
  nix develop --command cargo test --locked
  ```

- Run line and branch coverage through the nightly Nix development shell:

  ```bash
  nix develop .#coverage --command cargo llvm-cov --release --locked --all-features --no-fail-fast --branch --lcov --output-path lcov.info
  ```

- The flake also exposes `nix build .#coverage`, which runs the same
  branch-coverage configuration in a Nix build.
- Tests that use `RealDisplayState` depend on the current macOS display
  environment. Prefer `FakeDisplayState` for deterministic behavior when adding
  focused tests.

## Debugging

- When debugging build, test, toolchain, rust-analyzer, or coverage failures,
  reproduce through the Nix flake first so diagnostics use the same pinned
  inputs as normal development.
- Prefer `nix build .#default --print-build-logs` for package build failures,
  `nix build .#coverage --print-build-logs` for coverage failures, and
  `nix flake check --print-build-logs` for flake output issues.
- Use `nix develop` or `nix develop .#coverage` for interactive Cargo
  investigation after identifying which flake path is failing.

## Version Control

This repository is tracked with Jujutsu. Use `jj status` and `jj diff` for
status and review. Do not commit unless explicitly asked.
