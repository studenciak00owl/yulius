# 0288. Codex returns as a second harness in this fork

## Status

Accepted for the fork `studenciak00owl/yulius` only. Specified in
[SPEC-445](../../.ai/specs/SPEC-445-fork-codex-harness.md); fork task #3. Reverses upstream's
founder decision `F 2026-10-08` (SPEC-001; "prune Codex support and switching providers; we use
only Claude") for this fork's instance; supersedes
[ADR-0005](0005-claude-only-accounts.md)'s "Claude Code only" here, keeping its per-sandbox homes.
Upstream's specs are left as they are.

## Context

Upstream removed Codex on 2026-10-08 (commit `9f88975a`): the adapter, Codex homes with
`auth.json` write-back, its login and usage probe, provider switching, the transcript IR and
translation, its UI and tests. The fork's owner wants Codex back, working like Claude Code:
Codex accounts in the subscription pool, sign-in from the UI, tasks and automations that run
Codex, the native Codex TUI in the task terminal, its transcript observed, usage windows and
resume after a restart. 715 commits landed on `dev` since the removal; reverting it conflicts in
74 files. Codex CLI moved from 0.160.1 to 0.167.0 meanwhile.

## Decision

- Codex is `Harness::Codex` with `Provider::OpenAi`, re-implemented on today's code with the
  removed code as reference and checked against Codex CLI 0.167.0 (its help texts, its
  `rust-v0.167.0` source and the CLI itself on our generated files).
- A task keeps its harness for life. Rotation stays within the task's provider; there is no
  transcript translation and no cross-provider switch (the removed IR stays removed).
- Each (sandbox, account) has a Codex home `<sandbox>/codex/<account>` whose `sessions` links to
  the sandbox's shared `<sandbox>/codex/sessions`, so a resumed session finds its rollout on any
  Codex account. Yulius writes the home's `auth.json`, `config.toml` (file credential store,
  update check off, the worktree trusted) and `hooks.json` before each run through handles opened
  without following links. The home's `auth.json` is the agent's to write, so it goes back into
  the vault only as a genuine refresh: the same ChatGPT account, a newer `last_refresh` not dated
  ahead of the server's clock, never for an API-key account. A newer copy another run of the
  account left is kept at the next launch instead of being overwritten, and the file is removed
  once no run of the account is left in the sandbox. The agent's shell does not inherit
  `CODEX_HOME`, so a `codex` it runs does not get Yulius's hooks.
- The run is `codex --no-daemon -C <worktree> -c …` with Yulius's MCP as the `yulius mcp` stdio
  bridge; a sandbox MCP server's renamed variables pass through a `sh` wrapper, since Codex hands a
  server only variables of their own names. Codex names its own session: Yulius takes it from the
  SessionStart hook, else from the newest rollout of the worktree.
- A usage limit is the turn's `task_complete` error `usage_limit_exceeded`; the windows come from
  `token_count.rate_limits` and from `chatgpt.com/backend-api/wham/usage`. A resume after a limit
  asks Codex to continue, which it does not do by itself.
- A Codex account signs in with `codex login --device-auth` in the server-owned terminal, or is
  given an OpenAI API key, sealed as the `auth.json` Codex writes for it.
- Codex takes `/goal` only typed, never as its launch prompt: a Codex goal task gets its whole
  request and definition of done as its first message and is settled by its turns.

## Alternatives

- **Revert `9f88975a`**: 74 conflicted files against code that changed under them, and an adapter
  written for Codex 0.160.1; rejected by the request too.
- **Bring back translation between harnesses**: a task could move to whichever provider has
  capacity, at the cost of lossy transcripts and the IR's upkeep; the owner asked for none.
- **One Codex home per sandbox**: two tasks on different accounts in one sandbox would overwrite
  each other's `auth.json`.
- **Codex `exec --json` instead of the TUI**: no native terminal session to watch and type into,
  unlike Claude Code's.

## Consequences

- Codex runs get no in-sandbox redaction of tool output: Codex's PostToolUse hook cannot replace
  a result, and it has no shell prefix like `CLAUDE_CODE_SHELL_PREFIX`. Agent secrets still reach
  it only as placeholders, but a value copied into the worktree can reach the model.
- Yulius's edit gate (SPEC-426) does not see Codex's `apply_patch`; the shell guard does see its
  `Bash` tool.
- Claude-only features stay Claude-only for Codex tasks: goal tracking, typed openings with
  pasted images, the transcript step-back, the status line, sub-agent tails, session import and
  search of Codex sessions.
- Two runs of one account in different sandboxes each refresh its tokens; Codex rotates refresh
  tokens, so one copy can go stale until written back (upstream's R-02, unverified live).
- The fork's SPEC and ADR numbers may collide with upstream's later ones when upstream is merged.
