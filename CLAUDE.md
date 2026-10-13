@AGENTS.md

## Claude Code specifics

- `.claude/settings.json` sets `attribution.commit` and `attribution.pr` to empty strings so Claude Code adds no co-author trailer or footer. Keep it that way, and don't add attribution by hand either.
- Load `docs/style.md` into context before the first edit of a session; `AGENTS.md` summarizes it but does not replace it.
- When work spans more than one phase step, plan it first and check the plan against `ARCHITECTURE.md`'s invariants before editing.
- When you start subagents, give them the hard rules from `AGENTS.md` and tell them to read `docs/style.md`. Don't rely on them having this file in context.
