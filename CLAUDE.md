# Claude Guidance

## Veilid Cross-Repo Upgrades

For Veilid upgrades, use the repo-local skill at `.claude/skills/veilid-upgrade/SKILL.md` and read `.claude/skills/veilid-upgrade/references/dependency-map.md` before editing.

The expected dependency chain is:

```text
veilid -> veilid-iroh-blobs -> save-dweb-backend -> save-rust
```

This checkout is expected to sit beside `../veilid-iroh-blobs` and `../save-dweb-backend`. Upgrade one repo at a time, wait for review/merge, tag the upstream release, then move to the next downstream repo.

Do not combine the OpenArchive fork-origin migration with a Veilid version bump unless the user explicitly asks.
