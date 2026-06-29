---
name: veilid-upgrade
description: Upgrade Veilid across the OpenArchive Rust dependency chain. Use when asked to upgrade Veilid, bump Veilid, move to a new Veilid version, refresh veilid-core or veilid-tools tags, or run the cross-repo Veilid release cascade for veilid-iroh-blobs, save-dweb-backend, and save-rust.
---

# Veilid Upgrade

Upgrade Veilid one repository at a time, in dependency order:

1. `../veilid-iroh-blobs`
2. `../save-dweb-backend`
3. current `save-rust`

Read `references/dependency-map.md` before editing. It contains the repo-specific pins, git URLs, and verification commands. Treat it as a map, not as a substitute for inspecting the current files.

## Ground Rules

- Locate dependency entries by package key or `rg`, never by hardcoded line number.
- Keep fork-origin migration separate from a Veilid version bump unless the user explicitly asks to combine them.
- Pause after each PR is opened and ask the human to review and merge before tagging and continuing downstream.
- Verify each repo's default branch, origin, and push/tag permissions before creating PRs or tags.
- Default to a patch version bump for routine dependency-only upgrades; use a minor bump only when the changelog or API impact warrants it.

## One-Time Fork Migration

The chain historically depended on personal upstream forks:

- `RangerMauve/veilid-iroh-blobs`
- `tripledoublev/iroh`

OpenArchive forks now exist and should be the long-term dependency sources:

- `OpenArchive/veilid-iroh-blobs`
- `OpenArchive/iroh`

Repoint references in standalone PRs before or after a Veilid upgrade. Do not mix that origin migration into the Veilid bump unless the user asks.

Until a repo's migration PR is merged, operate against the current upstreams listed in that repo's manifest.

## Step 0: Verify The Target Veilid Tag

Discover the target tag live. Prefer machine-readable sources over GitLab HTML pages:

```bash
git ls-remote --tags https://gitlab.com/veilid/veilid.git
curl -fsS 'https://gitlab.com/api/v4/projects/veilid%2Fveilid/repository/tags?per_page=10'
curl -fsS https://gitlab.com/veilid/veilid/-/raw/main/CHANGELOG.md
```

Record the target tag, the target commit hash, and the changelog entry.

Run the hickory/iroh gate before editing:

1. Inspect the target tag's `veilid-tools/Cargo.toml` hickory dependency or scan the changelog for hickory changes.
2. If Veilid still forces the resolver constraint that required `hickory-resolver = "=0.25.2"`, keep the hickory pins and iroh patches unchanged.
3. If Veilid relaxed the constraint, drop the hickory pins and iroh patches only as a separate, well-tested change unless the user explicitly asks to combine it.
4. For routine bumps, default to keeping the existing hickory pins and iroh patches.

## Steps 1-3: Upgrade Each Repo

For each repo in order:

1. Inspect the current manifest:

```bash
rg 'veilid-core|veilid-tools|veilid-iroh-blobs|save-dweb-backend|hickory-resolver|patch.crates-io|tripledoublev|RangerMauve|OpenArchive' Cargo.toml
```

2. Edit `Cargo.toml`:

- Update every `veilid-core` and `veilid-tools` git tag to the target Veilid tag.
- Update downstream git tags only after the upstream repo's PR has merged and its release tag has been pushed.
- Update all occurrences found by search, including target-specific dependency stanzas.
- Bump the package version.
- Leave hickory and iroh patch entries untouched unless Step 0 explicitly calls for a separate compatibility cleanup.

3. Refresh the lockfile with package-aware commands from `references/dependency-map.md`.

- Run `cargo update -p <pkg>` only for packages present in that repo.
- If Cargo refuses a package target because of source ambiguity or graph absence, run a full lockfile refresh with `cargo update` or let `cargo build` resolve it.
- Inspect `Cargo.lock` to confirm the new Veilid commit hash and any downstream git tag/hash landed.

4. Build and test.

- Use a normal fresh build in the current tree. Do not run `cargo clean` unless stale artifacts or dependency resolution make it necessary.
- Discover smoke test names before invoking exact filters:

```bash
cargo nextest list
cargo test -- --list
```

5. Open a PR and pause.

- Verify default branch and permissions with `gh repo view --json defaultBranchRef` and git remote checks.
- Create a branch named like `chore/veilid-0.5.x`.
- Commit the repo-local upgrade.
- Open a PR to the repo's default branch.
- Stop and ask the human to review and merge.

6. After merge, tag the release before continuing downstream:

```bash
git pull --ff-only
git tag v<new-version>
git push origin v<new-version>
```

The next repo must not point at an upstream tag until that tag exists remotely.

## Final save-rust Release

Run the same loop for `save-rust`. Use the repo convention for the release commit, for example:

```text
chore: release v0.2.x
```

Tag and push the final `save-rust` release after its PR merges.

## Verification

- Each repo builds and tests before its PR.
- `Cargo.lock` in each repo records the expected Veilid commit hash for the target tag.
- Downstream repos point at the freshly pushed upstream release tags.
- In `save-rust`, verify the new `save-dweb-backend` tag/hash and Veilid hash in `Cargo.lock`.
- Search all three manifests for stale old tags, replacing `OLD_VEILID_REGEX` with the previous Veilid version escaped for regex, for example `0\.5\.5`:

```bash
OLD_VEILID_REGEX='0\.5\.x'
rg "${OLD_VEILID_REGEX}|v${OLD_VEILID_REGEX}" ../veilid-iroh-blobs/Cargo.toml ../save-dweb-backend/Cargo.toml Cargo.toml
```
