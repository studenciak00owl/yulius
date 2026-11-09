---
yulius-task: 49862d3b-f280-4116-a8eb-fc3b1a415393
status: in-progress
---

# SPEC-445: Codex runs as a second harness again (fork only)

Status: in progress
Date: 2026-11-09
Owner: Yulius task #3 of the fork `studenciak00owl/yulius`
Scope authorization: the fork's owner, verbatim: "Bring back Codex (OpenAI Codex CLI) as a second
harness in this fork, working the same way Claude Code does: Codex accounts in the subscription
pool, login from the UI, a task can be created with harness Codex, the native Codex session runs in
the task terminal, observation of its transcript, usage windows, resume after restart."
Fork only: upstream removed Codex on 2026-10-08 (commit `9f88975a`, founder decision
`F 2026-10-08` in SPEC-001). This fork reverses that decision for its own instance; upstream's
SPEC-001 stays as it is. ADR:
[0288](../../docs/decisions/0288-fork-codex-returns-as-a-second-harness.md).

## Goal

A person of the fork's instance can connect Codex accounts (ChatGPT sign-in by device code shown
in the UI, or an OpenAI API key), create a task or an automation that runs Codex, and watch the
native Codex TUI in the task terminal. Yulius reads the Codex rollout for turns, usage and rate
limits, shows the accounts' usage windows, moves a task to another Codex account when one is at its
limit, resumes the same Codex session after a restart or a switch, and keeps the credentials Codex
refreshes during a run. A task never changes harness.

## Context

- Upstream commit `9f88975a` removed: the Codex adapter (`yulius-harness/src/codex.rs`: launch
  plan, `CODEX_HOME` homes, `hooks.json`, rollout observation and discovery, the `--no-daemon`
  probe), Codex login and `auth.json` write-back, the Codex usage probe, Codex session import, the
  transcript IR and cross-harness translation, harness proposals, the UI's harness choice and the
  fake agent's Codex transcripts. 715 commits landed on `dev` after it; a revert conflicts in 74
  files, so this is a re-implementation on today's code with the removed code as reference.
- Since then the Claude path grew: hook holds (ADR-17), the PostToolUse redaction and PreToolUse
  guard hooks, the status line usage feed, goal mode with `/goal` launch prompts (SPEC-117),
  typed openings (SPEC-030), the transcript step-back over a limit (SPEC-034), sub-agent tails,
  recounts (SPEC-091) and session drift by hook (SPEC-281). Those stay Claude-only unless a
  requirement below says otherwise.
- Rotation (`yulius-domain/src/rotation.rs`) already keeps candidates within the task's provider
  (`task_provider`), and accounts already carry a provider.
- The composer already groups models by provider and sends `harness` when the chosen model's
  provider runs another harness (`models.js`, `taskview.js` `bodyFor`).
- Codex CLI checked: 0.167.0 (npm `@openai/codex`, source tag `rust-v0.167.0`).
  `codex --help`, `codex exec --help`, `codex login --help` and `codex resume --help` were read;
  the rollout, hook and auth formats were checked in the 0.167.0 source (Evidence).

## Requirements

| ID | Requirement | Evidence |
| --- | --- | --- |
| CDX-001 | `Harness::Codex` (`codex`) and `Provider::OpenAi` (`openai`) exist; creating a task or an automation with harness `codex`, and an account with provider `openai`, is accepted; anything else stays 422. | `yulius-domain` harness tests; `tests/automations.rs`, `tests/task_delete.rs`, `tests/accounts.rs` |
| CDX-002 | A Codex run is `codex [--no-daemon] -C <worktree> -c <config>… [-m <model>] [--dangerously-bypass-approvals-and-sandbox] --dangerously-bypass-hook-trust [resume <id>] [prompt]` with `CODEX_HOME=<sandbox>/codex/<account>`, the account's `auth.json` and Yulius's `config.toml` and hooks written there (mode 0600), the Yulius MCP as the `yulius mcp` stdio bridge, the sandbox's MCP servers with their values only in the secret environment, and no secret in argv. | `yulius-harness/tests/launch.rs` |
| CDX-003 | Codex homes are per (sandbox, account) and share one `sessions` directory per sandbox, so a resumed session finds its rollout whichever account runs it. A session's rollout is found by its id; a new session's id is adopted from its SessionStart hook, else from the first rollout for the worktree created after the run started. | `tests/paths.rs`; `tests/codex.rs` |
| CDX-004 | The rollout is observed: session meta, user turns (not injected context), turn completion, token usage (per response, so a resumed run counts only its own), rate-limit windows (recorded on the account) and a reached rate limit (a usage limit: the task rotates or waits). Unknown types are ignored and malformed lines counted. | `yulius-harness/tests/observe.rs` with synthetic fixtures `tests/fixtures/codex/*` |
| CDX-005 | A Codex account signs in from the Accounts page by `codex login --device-auth` in a server-owned terminal (the device URL and code shown, credentials kept in `auth.json` of a temporary home) or by an OpenAI API key given in the UI; the credential is a validated `auth.json` sealed in the vault. | `yulius-harness/tests/login.rs`; `tests/accounts.rs` |
| CDX-006 | After a Codex run ends, the run's `auth.json` replaces the stored one only as a genuine refresh (same ChatGPT account, newer `last_refresh` not dated ahead; never an API-key account or a signed-out one), so a refreshed token is kept and a forged file is not. A newer copy another run of the account left is kept at launch; the file is removed once no run of the account is left in the sandbox. | `yulius-harness/tests/login.rs`; `tests/codex.rs`; `supervisor.rs` unit tests |
| CDX-007 | Codex hooks reach Yulius as Claude Code's do (`yulius hook codex <event>`): SessionStart (briefing, session id), UserPromptSubmit, Stop (turn boundary, gate, holds) and PreToolUse; a hook of another harness than the run's is refused. | `tests/agent_hooks.rs` |
| CDX-008 | A Codex account's usage windows come from `chatgpt.com/backend-api/wham/usage` (read-only) and from the rate limits in its runs' rollouts; an API-key account has no windows. | `wiring/usage.rs` tests |
| CDX-009 | Rotation picks only accounts of the task's provider; a Codex task at its limit moves to another Codex account by native resume, and waits when none has capacity. A task never switches harness. | `yulius-domain` rotation tests; `tests/codex.rs` |
| CDX-010 | The UI offers Codex: the composer's model menu has an OpenAI group (its choice sends `harness: codex`), automations show and take a harness, the Accounts page adds a Codex account (device sign-in or API key), and the mock serves the same. | e2e and mock checks |

## Plan

- [x] Codex harness and OpenAI provider accepted by the domain
- [x] Codex rollout observed with synthetic 0.167 fixtures
- [x] Codex launch plan checked against Codex CLI 0.167
- [x] Codex homes, session discovery and auth write-back in the supervisor
- [x] Codex hooks reach Yulius like Claude Code's
- [x] Codex accounts sign in by device code or API key, with usage windows
- [x] Fake agent speaks Codex and integration tests cover a Codex task
- [x] UI offers Codex in the composer, automations and Accounts
- [x] Fork spec, ADR and documents describe the second harness
- [ ] Independent review answered and branch pushed

## Acceptance criteria

- AC-445-1: `POST /api/sandboxes/{id}/tasks` with `harness: codex` creates a Codex task that runs
  the planned `codex` command in its terminal (fake agent in tests).
- AC-445-2: a Codex run's rollout drives the task's turns, usage and limit; at a rate limit the
  task resumes the same session on another Codex account, or waits when none has capacity.
- AC-445-3: a restart reattaches the run and watches its rollout; a later Continue resumes it
  natively (`codex resume <id>`).
- AC-445-4: an administrator adds a Codex account from the UI and sees the device code, or saves
  an API key; the credential is stored sealed and never shown.
- AC-445-5: a refreshed `auth.json` left by a run is written back to the account.

## Test plan

- Unit: domain parsing; Codex observation, launch plan, login helpers and rollout paths in
  `yulius-harness`; usage parsing in `yulius`.
- Integration (`crates/yulius/tests`, fake agent extended with `--transcript-kind codex`): a Codex
  task's launch, rollout watch, rotation at a limit, resume, auth write-back, hooks; accounts with
  provider `openai` (API key path); refusals for unknown harnesses.
- No test calls OpenAI: credentials and rollouts are synthetic.

## Decisions

- Re-implemented, not reverted (74 conflicting files); the removed code is the reference.
- No transcript translation and no cross-provider switch: a task keeps its harness (the owner's
  scope, item 4). The removed IR and translation stay removed.
- Codex session ids are adopted from the SessionStart hook first (Codex has no `--session-id`),
  with the rollout `session_meta` scan as the fallback.
- Claude-only features that need Codex counterparts not checked live (goal launch prompts, typed
  openings with images, step-back, status line, sub-agent tails) stay Claude-only; a Codex goal
  task gets its request as its first prompt.

## Evidence

(filled when done)
