---
yulius-task: 49862d3b-f280-4116-a8eb-fc3b1a415393
status: done
---

# SPEC-445: Codex runs as a second harness again (fork only)

Status: implemented (live Codex account checks pending)
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
| CDX-011 | A Codex account takes an imported `auth.json` of a ChatGPT sign-in made elsewhere (`codex login` with a separate `CODEX_HOME`): at creation (`auth_json`) or for an existing account (`POST /api/admin/accounts/{id}/import`); only a full ChatGPT sign-in, sealed, never returned or logged; its refreshes are written back (CDX-006). Detailed in [SPEC-446](SPEC-446-codex-import-auth-json-codex-cli-in-sand.md) IMP-001..004. | `tests/accounts.rs`; `tests/codex.rs`; `e2e/tests/codex.e2e.ts` |
| CDX-012 | Codex runs and sign-ins start the pinned Codex CLI Yulius installs from npm into `<data>/codex-cli` (sha512 pinned in the source), never a `codex` the sandbox may lack (ADR-0289; SPEC-446 CLI-001, CLI-002). | `tests/codex_cli.rs`; `tests/codex.rs` |
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
- [x] Independent review answered and branch pushed
- [x] Codex accounts take an imported auth.json and runs get the provided Codex CLI (SPEC-446)

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

- The run's `auth.json` is trusted only as a refresh of the same sign-in (`tokens.account_id` and
  the `id_token`'s `sub`), and removed after the account's last run only when it equals the
  vault's copy. Known limit: one account used at once in two sandboxes keeps two copies of its
  rotating refresh token, and one of them can go stale (upstream's R-02; not verified live).

## Evidence

Run on 2026-11-09 on branch `feat/codex-harness` (from `upstream/dev` `da0e55a4`).

- Codex CLI 0.167.0 (`npm i @openai/codex@0.167.0`): `codex --help`, `codex exec --help`,
  `codex login --help`, `codex resume --help` read; the rollout, hook, auth, MCP and shell-policy
  formats checked in the `rust-v0.167.0` source (`codex-rs/{protocol,hooks,login,rollout,config,
  core}`). The real CLI accepted Yulius's planned `-c` overrides (Yulius MCP stdio bridge, the
  `sh -c` wrapper with `env_vars`, a bearer HTTP server, `model_reasoning_effort`), its
  `config.toml` (file credentials, no update check, `[shell_environment_policy]`, the trusted
  worktree) and `hooks.json` (`codex --no-daemon … mcp list` lists them; no parse error), and the
  API-key `auth.json` (`codex login status`: "API key configured").
- `cargo fmt --all -- --check`: clean. `cargo clippy --workspace --all-targets --locked -- -D
  warnings`: clean.
- Unit tests: `cargo test -p yulius-domain -p yulius-contracts -p yulius-harness -p yulius-app -p
  fake-agent` and `cargo test -p yulius --lib`: all pass (Codex: observer, launch plan with the
  executed `sh -c` wrapper, login and write-back rule, rollout paths and discovery, usage probe
  against a local server, sessions link, no-follow reads, `CreateAccount` redaction).
- Integration (`cargo test -p yulius --test main -- codex:: accounts:: agent_hooks:: claude_gate::
  task_delete:: automations:: supervision:: pool_supervision:: subscription_pool account_usage::
  run_metrics:: restart:: metrics_recount:: cockpit::`): 200 passed, 1 ignored, 1 failed under
  load, `cockpit::an_idle_superagents_strays_are_reaped_after_the_bound` (timing-bound, Cockpit
  code untouched), which then passed 5/5 alone. The 5 `codex::` tests pass 3 runs in a row.
- e2e (`cd e2e && npx e2e run`, Node 22.23): `codex.e2e.ts` 3/3; `subscriptions`,
  `subscription-names`, `automations`, `new-task`, `claude-connect`, `consistency`,
  `onboarding-flow`: all pass. Full suite (77 files, run while cargo tests and the review ran):
  873 passed, 1 skipped, 7 timed out (`git`, `intake`, `responsive` ×3, `subscriptions`,
  `update`); those five files then passed alone, 91/91.
- `python3 scripts/check_repository.py`: passed. `spec_check`: valid.
- `scripts/dev security`: **not run** (exit 2: no `VIDOC_API_KEY` on this instance).
- Independent review (fresh read-only session, four rounds): blocker (MCP wrapper without
  `sh -c`), write-back trust, concurrent refreshes, nested `codex` hooks, discovery order, model
  check, probe reset, sessions link, removal of `auth.json` and its races all fixed test-first;
  final verdict APPROVE.
- Not verified (needs a live Codex account): a real `codex login --device-auth`, a real rollout
  and hook payloads of the TUI, token refresh and write-back, `wham/usage`, the usage-limit
  error and a "Resume paused goal?" popup after a resume.
