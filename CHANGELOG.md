# Changelog

## 2026-06-09

- Bump `save` crate version to `0.2.6`.
- Bump `save-dweb-backend` dependency to `v0.3.9`.
- Defer media body downloads during refresh so file metadata remains available when large media transfers are slow or fail.
- Return an empty media list for empty repositories instead of surfacing a DHT root-hash error.

## 2026-05-31

- Bump `save` crate version to `0.2.5`.
- Patch `actix-http` to `3.12.1` to address `GHSA-xhj4-vrgc-hr34`.
- Remove stale personal author metadata from `Cargo.toml`.

- Bump `save-dweb-backend` dependency to `v0.3.7`.
- Bump `save` crate version to `0.2.4`.

## 2026-02-17

- Upgrade to `veilid-core` v0.5.2.
- Bump `save-dweb-backend` dependency to `v0.3.3`.
- Update CI to run a focused smoke-test subset in PR workflows for faster feedback.

## 2026-02-03

- Upgrade to `veilid-core` v0.5.1, applying the temporary `fix-underflow` patch while the upstream bugfix is under review.
- Adopt `cargo-nextest` with retries in CI to stabilize Veilid/DHT-related tests.
