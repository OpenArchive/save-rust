# Save DWeb Stack — Architecture Overview

Orientation for a new maintainer: what the Save DWeb stack is, how the Rust repos
fit together, and how they build, test, and ship. Veilid upgrades have their own
runbook (`.claude/skills/veilid-upgrade/`); this doc covers everything else.

Scope: `save-rust` and `save-dweb-backend`, plus the role of `veilid-iroh-blobs`.
Consumer apps are covered only as far as where the build artifact goes.

---

## 1. What "Save" is

Save DWeb is decentralized, server-less media archiving. Peers form encrypted
groups and replicate content to each other over a P2P network. There is no
separate server to run or pay for: every device is both client and server, and
all data is encrypted under a per-group secret.

These Rust repos are the engine — P2P discovery, encryption, blob replication —
plus a thin bridge that exposes it to a mobile app over a local HTTP API.

---

## 2. The stack at a glance

Each layer depends on the one above it, pinned by git tag. Upgrades cascade
top-to-bottom, one repo at a time.

```text
veilid                  gitlab.com/veilid/veilid           tag v0.5.5
  │   P2P DHT, private routing, public-key crypto
  ▼
veilid-iroh-blobs       github.com/OpenArchive/...          tag v0.3.8  (Cargo.toml says 0.3.7 — see note)
  │   bridges Veilid private routes <-> iroh-blobs; blobs replicate over Veilid tunnels
  ▼
save-dweb-backend       github.com/OpenArchive/...          tag v0.3.12
  │   the engine: groups, repos, members, DHT records, replication
  │   pure-Rust library crate (+ daemon CLI, + veilid-probe diagnostic)
  ▼
save-rust  (pkg `save`) github.com/OpenArchive/save-rust    v0.2.9
  │   the bridge: JNI entry points + actix-web HTTP API; builds libsave.so / xcframework
  ▼
consumer apps           Save-app-android (Kotlin "snowbird"), iOS (xcframework)   — out of scope
```

The repos are developed side by side; build scripts and the upgrade tooling
assume this layout:

```text
OpenArchive/
  veilid-iroh-blobs/
  save-dweb-backend/
  save-rust/            ← you are here
  Save-app-android/     ← build-android.sh writes .so files into this
```

> **Note — v0.3.8 / 0.3.7.** `OpenArchive/veilid-iroh-blobs` tag `v0.3.8` points
> at a commit whose `Cargo.toml` reads `version = "0.3.7"`. This is intentional —
> downstream consumes by tag — and should stay as-is.

---

## 3. save-dweb-backend — the engine

`../save-dweb-backend` (package `save-dweb-backend` v0.3.12) holds the
decentralized logic. It is a pure-Rust library crate (the FFI layer lives in
`save-rust`), plus a `main.rs` daemon CLI and a feature-gated `veilid-probe`
diagnostic binary. `save-rust` consumes it as a normal Cargo dependency.

Its `README.md` (~525 lines) is the authoritative protocol reference; read it for
wire-level detail. The core concepts:

- **Group** — the unit of sharing and trust. An ED25519 keypair plus a random
  `chacha20poly1305` secret, stored as a Veilid DHT record. Subkey 0 is the
  encrypted group name; subkeys 1+ are member repo keys. Anyone with the secret
  is trusted; everyone else is locked out.
- **Repo** — a per-member data store, its own DHT record. Subkey 1 is the
  file-collection hash (CBOR `HashMap<path, Blake3 hash>`); subkey 2 is the
  member's current Route ID. Writable with the secret key, read-only with the
  public key.
- **Member / discovery** — peers resolve a group's member repos from the DHT,
  read each member's Route ID and collection hash (encrypted), and open tunnels
  to fetch content.
- **Tunnels / replication** — one-way AppCalls over private Veilid routes. Blob
  transfer uses an ASK/HAS/DATA/DONE byte protocol; the opening PING's bytes
  spell "SAVE" on a phone keypad. iroh handles content addressing and Blake3
  verification; `veilid-iroh-blobs` carries the bytes over Veilid.

Module map (`../save-dweb-backend/src/`):

| File | Role |
|------|------|
| `backend.rs` | Lifecycle (start/stop), Veilid + iroh init, group create/join, state persistence |
| `group.rs` | Group struct; DHT create/join/watch, member discovery, replication, encrypt/decrypt |
| `repo.rs` | Repo struct; file upload/download, collection-hash management, route updates, write-permission checks |
| `rpc.rs` | RPC service/client for remote control (join/list/remove groups) over Veilid AppCalls |
| `common.rs` | Veilid init, route creation (`make_route`), AEAD helpers, test setup |
| `main.rs` | CLI daemon: `join` / `remove` / `list` / `start` |
| `lib.rs` | Public API + ~30 integration tests |
| `bin/veilid-probe.rs` | Startup profiler (attach → route → upload phases); needs `--features probe` |

---

## 4. save-rust — the bridge

`save-rust` (package `save`, v0.2.9) is a thin wrapper that makes the engine
usable from a mobile app. It does two things:

1. **JNI bridge** — C-ABI entry points the Android app calls.
2. **HTTP API** — a local actix-web server for group/repo/media operations.

From one crate (`crate-type = ["staticlib", "cdylib", "rlib"]`) it builds:

- `libsave.so` / `libsave.a` — linked into the Android app (and iOS via xcframework).
- `save-server` — a desktop dev/test binary (`src/bin/server.rs`).

Module map (`src/`):

| File | Role |
|------|------|
| `lib.rs` | Library root + ~12 integration tests (the P2P/refresh suite) |
| `server.rs` | actix-web setup; lazy backend init (HTTP comes up first, backend initializes in the background); binds `127.0.0.1:8080` and a Unix socket |
| `groups.rs` / `repos.rs` / `media.rs` | Handlers for the `/api/groups → /repos → /media` hierarchy |
| `models.rs` | `SnowbirdGroup` / `SnowbirdRepo` / `SnowbirdFile` and conversions from backend types |
| `android_bridge.rs` | JNI `#[no_mangle]` entry points: `initializeRustService`, `startServer`, `stopServer` |
| `jni_globals.rs` | JNI static state (`JAVA_VM`, `CLASS`, init guard) |
| `error.rs` | `AppError` → HTTP mapping (503 while initializing, 500 otherwise) |
| `logging.rs` | Log macros + Android log bridge |
| `actix_route_dumper.rs` | Middleware that logs routes on startup |
| `bin/server.rs` | `save-server` dev binary entry point |

HTTP endpoints (schemas in [`API.md`](../API.md)): `GET /status`, `GET /health`,
`GET /health/ready`, `POST /api/memberships`, and the nested
`/api/groups[/{id}[/refresh]]`, `/repos[/{id}]`, `/media[/{file}]` (list / create
/ upload / download / delete).

The backend initializes asynchronously, so endpoints return 503 until
`/health/ready` reports it is up. The Android app polls `/health/ready` before
issuing requests.

Build scripts (repo root):

- `build-android.sh` — installs the three Android Rust targets, runs `cargo-ndk`
  `--release`, and writes `.so` files into
  `../../Save-app-android/app/src/main/jniLibs/{abi}/` for `arm64-v8a` (primary),
  `armeabi-v7a`, and `x86_64`.
- `build-apple.sh` / `build-xcframework.sh` — iOS static libs / xcframework. The
  iOS app is in a separate repo; this path is exercised less than Android.

---

## 5. veilid-iroh-blobs — the tunnel layer

`../veilid-iroh-blobs` (OpenArchive fork) lets iroh-blobs replicate over Veilid
private routes — "Privately replicate blobs over Veilid using Iroh." That routing
is what keeps the stack private.

You touch this repo during a Veilid upgrade (it is the first link in the cascade)
or when re-pointing the patched iroh fork. The iroh fork and the
`hickory-resolver = "=0.25.2"` pin are explained in the
`.claude/skills/veilid-upgrade/` runbook.

---

## 6. Build & run

Standard Cargo workspaces. Veilid needs a large thread stack for DHT work.

```bash
cargo build                                          # or --release

cargo run --bin save-server                          # save-rust dev server
cargo run -- start                                   # save-dweb-backend daemon (join/list/remove)
cargo run --features probe --bin veilid-probe -- --only-startup   # startup profiler
```

Android artifact: `./build-android.sh` (needs `ANDROID_HOME` / `ANDROID_NDK_HOME`
and `cargo install cargo-ndk`); output lands in the sibling `Save-app-android`
checkout's `jniLibs`.

Key environment variables:

- `RUST_MIN_STACK=8388608` — required for tests; Veilid overflows the default stack.
- `SAVE_VEILID_LOCAL_TEST_MODE=1` — disables UPnP / address-change detection for stable runs.
- `SAVE_VEILID_TEST_NETWORK_KEY`, `SAVE_VEILID_TEST_BOOTSTRAP` — override test network password / bootstrap nodes.
- `SAVE_WORKER_COUNT` — actix worker count (default 1).

---

## 7. CI/CD

Both repos use one workflow, `.github/workflows/lint_and_test.yml`, on
`ubuntu-latest`: lint plus a `cargo-nextest` pass. Releases are manual (§9).

- **`save-rust`** runs on PRs and pushes to `main`. Both triggers run the same
  3-test smoke subset (`--profile ci-virtual -E 'test(basic_test) |
  test(test_health_endpoint) | test(test_upload_list_delete)'`). The P2P tests
  (`test_replicate_group`, `test_join_group`, the refresh tests) run locally via
  `cargo nextest run`.
- **`save-dweb-backend`** runs on PRs only, full suite each time. This is where
  the stack's P2P/DHT coverage runs in CI.

| | save-rust | save-dweb-backend |
|---|---|---|
| `cargo fmt --check` | — | ✅ |
| `cargo clippy --all-targets --all-features -- -D warnings` | ✅ | ✅ |
| Triggers | `pull_request` + push to `main` | `pull_request` |
| Test scope | smoke subset (`basic_test`, `test_health_endpoint`, `test_upload_list_delete`) | full suite (`--test-threads=1 --retries=3`) |
| P2P suite | local `cargo nextest run` | every PR |
| Env | `RUST_MIN_STACK=8388608`, `SAVE_VEILID_LOCAL_TEST_MODE=1` | `RUST_MIN_STACK=8388608` |

---

## 8. Testing & flakiness

Veilid DHT/P2P tests are flaky because they exercise a real P2P network — they
depend on a node attaching and on DHT propagation timing, both nondeterministic.
Both repos handle this the same way: run serially (`#[serial]`,
`test-threads = 1`) and retry with backoff. Retry policy is in each repo's
`.config/nextest.toml`:

- **save-dweb-backend:** 600s timeout, 3 retries, serial.
- **save-rust:** 60s×10 timeout with exponential retries; per-test overrides give
  the P2P tests their own budgets; a faster `ci-virtual` profile for PRs.

Known flaky tests (expect occasional first-pass failures that pass on retry):

| Repo | Test | Why |
|------|------|-----|
| save-rust | `test_replicate_group`, `test_join_group` | two backends discover + converge over DHT |
| save-rust | `test_refresh_joined_group`, `test_refresh_member_joined_before_owner_uploads` | refresh waits on peer convergence |
| save-rust | `test_refresh_empty_group` | `/refresh` right after startup hits `TryAgain: offline` before attach; no retry override (known gap) |
| save-dweb-backend | `download_hash_from_peers_test`, `peers_have_hash_test`, `test_rpc_client`, `test_auto_create_repo_on_join_group` | P2P replication / RPC over the live DHT |
| save-dweb-backend | `sending_message_via_private_route`, `live_dht_valuechange_delivered_for_watched_record` | live DHT-watch delivery; `#[ignore]`d, experimental |

A common signature is the relay lookup error (`"couldn't look up relay for
inbound relay: VLD0:..."`) — a transient Veilid condition seen locally and in CI.
Re-run before assuming a regression.

---

## 9. Releases & versioning

Independent semver per repo: `save-rust` on `0.2.x`, `save-dweb-backend` and
`veilid-iroh-blobs` on `0.3.x`. Releases are manual:

1. Bump `version` in `Cargo.toml` (and `CHANGELOG.md` where the repo keeps one).
2. Commit (`chore: release vX.Y.Z`).
3. After merge, tag `vX.Y.Z` and push the tag.
4. Publish a GitHub Release from the tag — a pushed tag alone does not create one.

Downstream pins upstream by tag, so the order is fixed: `veilid-iroh-blobs` →
`save-dweb-backend` → `save-rust`. The full cascade is in the `/veilid-upgrade`
skill. Most version bumps in this project's history come from a Veilid bump.

---

## 10. Access & accounts

- **GitHub `OpenArchive`:** `save-rust`, `save-dweb-backend`, `veilid-iroh-blobs`,
  `iroh` fork — push/tag/release rights on all four.
- **GitLab `veilid/veilid`:** read access for tags/changelog.
- **Consumer:** `OpenArchive/Save-app-android` — `build-android.sh` writes into
  its `jniLibs`; the integration is called "snowbird" (`SnowbirdBridge` /
  `SnowbirdService`).
- **Remote quirk:** a local `veilid-iroh-blobs` checkout may have `origin` =
  `RangerMauve/veilid-iroh-blobs` with OpenArchive as a separate `openarchive`
  remote. Confirm with `git remote -v` and push tags to the release remote.

---

## 11. Map of the docs

| Document | For |
|----------|-----|
| `docs/ARCHITECTURE.md` (this file) | The overview |
| `.claude/skills/veilid-upgrade/` | Veilid upgrade runbook + per-repo dependency map |
| [`API.md`](../API.md) | HTTP endpoint schemas |
| [`README.md`](../README.md) | Quick start, build, test commands |
| `../save-dweb-backend/README.md` | Wire-level protocol reference |

---

## 12. Open questions & risks

- **Flaky-test tail.** Retries keep CI green but the timing stays sensitive.
  `test_refresh_empty_group` has no retry override; adding one in
  `save-rust/.config/nextest.toml` would match the other refresh tests.
- **DHT watches.** The two `#[ignore]`d save-dweb-backend tests cover live
  DHT-watch delivery, still experimental. Treat watch-based features as evolving.
- **iOS.** The Apple build scripts exist but the path sees less use than Android;
  revalidate before relying on it.
- **Compatibility boundary is temporary.** The iroh fork and `hickory-resolver`
  pin exist to bridge Veilid and Iroh. When a future Veilid relaxes the
  constraint, the fork can be retired — see the `/veilid-upgrade` skill.
