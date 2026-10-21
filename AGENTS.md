# Project skills

For reverse-engineering shipped binaries, packages, or application behavior,
read [.agents/skills/reverse-engineer-anything/SKILL.md](.agents/skills/reverse-engineer-anything/SKILL.md)
and follow the relevant references. Use normal repository tools for source analysis.

Keep REA skills and any tool registration scoped to this project. Do not use
upstream setup to create personal or machine-wide registrations. If REA tools
are needed but unavailable, continue supported work and explain the missing
project-local setup.

# Agent resource use

Reuse `CARGO_TARGET_DIR=/tmp/sdocx-agent-target` for agent validation and temporary
Rust harnesses. Set `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, and
`CARGO_PROFILE_TEST_DEBUG=0` for these checks. Coordinate builds instead of creating
per-agent target directories; normal user debug builds need not use these settings.

Keep native inputs, capture scripts and compact evidence receipts. Remove obsolete
build caches and reproducible render intermediates; avoid bulk artifact copies.
