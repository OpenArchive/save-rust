# Changelog

## 2026-02-17

- Upgrade to `veilid-core` v0.5.2.
- Bump `save-dweb-backend` dependency to `v0.3.3`.
- Update CI to run a focused smoke-test subset in PR workflows for faster feedback.

## 2026-02-03

- Upgrade to `veilid-core` v0.5.1, applying the temporary `fix-underflow` patch while the upstream bugfix is under review.
- Adopt `cargo-nextest` with retries in CI to stabilize Veilid/DHT-related tests.
