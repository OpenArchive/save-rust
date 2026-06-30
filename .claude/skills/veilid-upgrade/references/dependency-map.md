# Veilid Upgrade Dependency Map

Use this file after reading `SKILL.md`. Verify every fact against the live manifests before editing. Do not rely on line numbers.

## Repo Layout

The expected sibling checkout layout from `save-rust` is:

```text
../veilid-iroh-blobs
../save-dweb-backend
.
```

If a repo is missing, stop and ask the user whether to clone it, locate it, or skip that stage.

## Current Dependency Chain

```text
veilid tag
  -> veilid-iroh-blobs release tag
  -> save-dweb-backend release tag
  -> save-rust release tag
```

The downstream repo must not be updated until the upstream repo has a merged PR and a pushed release tag.

## Shared Compatibility Patch

The current compatibility workaround is:

- `hickory-resolver = "=0.25.2"`
- iroh patch entries under `[patch.crates-io]`

Keep this workaround during routine Veilid bumps unless the target Veilid changelog or `veilid-tools/Cargo.toml` proves the resolver constraint has been relaxed. If it has been relaxed, remove the workaround as a separate PR unless the user explicitly asks to combine it.

## Target Discovery

Use one or both machine-readable checks:

```bash
git ls-remote --tags https://gitlab.com/veilid/veilid.git
curl -fsS 'https://gitlab.com/api/v4/projects/veilid%2Fveilid/repository/tags?per_page=10'
```

Read the raw changelog:

```bash
curl -fsS https://gitlab.com/veilid/veilid/-/raw/main/CHANGELOG.md
```

If fetching from the network fails because of sandboxing, request approval for the same command rather than guessing the target.

## veilid-iroh-blobs

Path:

```bash
cd ../veilid-iroh-blobs
```

Important manifest entries to find with `rg`:

```bash
rg 'version =|veilid-core|hickory-resolver|patch.crates-io|iroh-net|tripledoublev' Cargo.toml
```

Expected dependency surfaces:

- package version, currently in the `0.3.x` line
- `veilid-core` git tag from `https://gitlab.com/veilid/veilid.git`
- `hickory-resolver = "=0.25.2"`
- `[patch.crates-io] iroh-net` from `https://github.com/OpenArchive/iroh.git` after fork-origin migration, or the current manifest URL if migration has not landed yet

Known release-history exception: `OpenArchive/veilid-iroh-blobs` tag `v0.3.8` contains the fork-origin migration but its `Cargo.toml` package version remains `0.3.7`. Do not move or force-push that pushed tag. Before the next wrapper release, check existing remote tags and choose the next unused package/tag version; this will likely be `0.3.9` unless the team explicitly accepts another mismatch.

Routine edit:

- Update all `veilid-core` tag occurrences to the target Veilid tag.
- Bump the package version, usually patch.
- Keep hickory and iroh patch entries unless the Step 0 gate says otherwise.

Lockfile refresh candidates:

```bash
cargo update -p veilid-core
cargo build
cargo test
```

If `cargo update -p veilid-core` is ambiguous or insufficient, use `cargo update`, then inspect `Cargo.lock`.

Release handoff:

- Open PR against the repo's default branch.
- After merge, tag `v<new veilid-iroh-blobs version>` and push it.
- `save-dweb-backend` cannot start until this tag exists remotely.

## save-dweb-backend

Path:

```bash
cd ../save-dweb-backend
```

Important manifest entries to find with `rg`:

```bash
rg 'version =|veilid-core|veilid-tools|veilid-iroh-blobs|hickory-resolver|patch.crates-io|tripledoublev|RangerMauve|OpenArchive' Cargo.toml
```

Expected dependency surfaces:

- package version, currently in the `0.3.x` line
- `veilid-core` git tag from `https://gitlab.com/veilid/veilid.git`
- `veilid-tools` git tag from `https://gitlab.com/veilid/veilid.git`
- `veilid-iroh-blobs` git tag from `https://github.com/OpenArchive/veilid-iroh-blobs` after fork-origin migration, or the current manifest URL if migration has not landed yet
- `hickory-resolver = "=0.25.2"`
- seven `[patch.crates-io]` iroh workspace crates from `https://github.com/OpenArchive/iroh.git` after fork-origin migration, or the current manifest URL if migration has not landed yet

Routine edit:

- Update all `veilid-core` and `veilid-tools` tag occurrences to the target Veilid tag.
- Update `veilid-iroh-blobs` to the tag pushed from the previous repo.
- Bump the package version, usually patch.
- Keep hickory and iroh patch entries unless the Step 0 gate says otherwise.

Lockfile refresh candidates:

```bash
cargo update -p veilid-core -p veilid-tools -p veilid-iroh-blobs
cargo build
cargo nextest run
```

If any `-p` target fails because the package is absent or ambiguous, run narrower `cargo update -p` commands or a full `cargo update`, then inspect `Cargo.lock`.

Test workflow — save-dweb-backend's CI runs the full suite, so run it all. `.config/nextest.toml` already serializes and retries the flaky Veilid P2P/DHT tests:

```bash
cargo nextest run
```

To narrow to specific tests, discover names first and pass them as a filter EXPRESSION with `-E`. A bare quoted string WITHOUT `-E` is a substring match and silently runs 0 tests:

```bash
cargo nextest list
cargo nextest run -E 'test(parse_url_rejects_malformed_url)'
```

Release handoff:

- Open PR against the repo's default branch.
- After merge, tag `v<new save-dweb-backend version>` and push it.
- `save-rust` cannot start until this tag exists remotely.

## save-rust

Path:

```bash
cd <save-rust checkout>
```

Important manifest entries to find with `rg`:

```bash
rg 'version =|save-dweb-backend|veilid-core|hickory-resolver|patch.crates-io|tripledoublev' Cargo.toml
```

Expected dependency surfaces:

- package name `save`
- package version, currently in the `0.2.x` line
- `save-dweb-backend` git tag from `https://github.com/OpenArchive/save-dweb-backend`
- `veilid-core` git tag from `https://gitlab.com/veilid/veilid.git`
- an additional Android-target `veilid-core` dependency
- `hickory-resolver = "=0.25.2"`
- seven `[patch.crates-io]` iroh workspace crates from `https://github.com/OpenArchive/iroh.git` after fork-origin migration, or the current manifest URL if migration has not landed yet

Routine edit:

- Update all `veilid-core` tag occurrences to the target Veilid tag.
- Update `save-dweb-backend` to the tag pushed from the previous repo.
- Bump the package version, usually patch.
- Keep hickory and iroh patch entries unless the Step 0 gate says otherwise.

Lockfile refresh candidates:

```bash
cargo update -p veilid-core -p save-dweb-backend
cargo build
cargo nextest run
```

If any `-p` target fails because the package is absent or ambiguous, run narrower `cargo update -p` commands or a full `cargo update`, then inspect `Cargo.lock`.

Smoke-test workflow — match CI's smoke set. Pass the filter as an EXPRESSION with `-E`; without `-E` the quoted string is a substring match and silently runs 0 tests:

```bash
cargo nextest list
cargo nextest run -E 'test(basic_test) | test(test_health_endpoint) | test(test_upload_list_delete)'
```

Only use exact test filters after confirming the names still exist.

Release:

- Use a release commit message like `chore: release v0.2.x`.
- Open PR against the repo's default branch.
- After merge, tag `v<new save-rust version>` and push it.

## Fork-Origin Migration

Do this only as a standalone change unless the user explicitly asks otherwise.

OpenArchive fork URLs:

- `https://github.com/OpenArchive/veilid-iroh-blobs`
- `https://github.com/OpenArchive/iroh.git`

Migration checklist:

1. Update the `veilid-iroh-blobs` git URL in `save-dweb-backend` (and any other direct consumer) to the OpenArchive fork.
2. Update every `[patch.crates-io]` iroh entry in all three repos to `OpenArchive/iroh`.
3. Refresh lockfiles so the source hashes repoint to the OpenArchive forks. Do NOT change the **Veilid version tags** — `veilid-core`/`veilid-tools` stay on their current `vX.Y.Z`.
4. Build and test all affected repos.
5. Keep each repo's diff focused on fork origins + lockfile source hashes — no Veilid version change.
6. Separate from the Veilid tags above, you will likely need to cut **new wrapper-crate release tags** so downstream repos can consume the merged fork-origin commits. Never move an existing pushed tag — instead bump the wrapper package `version` and tag the new version: e.g. release `veilid-iroh-blobs` (bump `version`, tag `v0.3.x`) after its migration merges, then `save-dweb-backend` (bump `version`, tag `v0.3.y`), repointing each downstream git tag the same way the Veilid cascade does. This is a wrapper-crate release bump, distinct from a Veilid version change.
