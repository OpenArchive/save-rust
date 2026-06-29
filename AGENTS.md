# Agent Guidance

## Veilid Cross-Repo Upgrades

For Veilid upgrades, read `.claude/skills/veilid-upgrade/SKILL.md` and `.claude/skills/veilid-upgrade/references/dependency-map.md`, then follow that runbook.

The `.claude/skills/...` directory is a Claude Code skill location, not a native Codex skill trigger. For Codex, this `AGENTS.md` file is the discoverability pointer.

Assume the sibling repo layout:

```text
../veilid-iroh-blobs
../save-dweb-backend
.
```

Upgrade in dependency order: `veilid-iroh-blobs`, then `save-dweb-backend`, then `save-rust`. Locate dependency pins with `rg` or package keys, not line numbers. Keep fork-origin migration separate from Veilid version bumps unless the user explicitly asks to combine them.
