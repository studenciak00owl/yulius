---
yulius-task: 4811641f-fe73-44ca-89d6-b6262b0fafca
status: done
---

# SPEC-446: Codex sign-in by imported auth.json, and the Codex CLI provided to sandboxes (fork only)

Status: implemented (live Codex account checks pending)
Date: 2026-11-09
Owner: Yulius task #4 of the fork `studenciak00owl/yulius`
Extends [SPEC-445](SPEC-445-fork-codex-harness.md) (requirements CDX-011 and CDX-012 are kept
there too) and [ADR-0288](../../docs/decisions/0288-fork-codex-returns-as-a-second-harness.md);
decision [ADR-0289](../../docs/decisions/0289-yulius-provides-the-pinned-codex-cli.md).
Delivered on the fork's branch `feat/codex-harness`, no pull request (the owner's instruction).

## Goal

The owner signs Codex in on his laptop (`CODEX_HOME=~/codex-yulius codex login`, ChatGPT in the
browser) and brings that `auth.json` into a Codex account, instead of the device-code flow that
needs a ChatGPT setting he does not want to depend on. And a Codex task never fails with
`codex: not found`: Yulius provides the pinned Codex CLI to every Codex run the way it provides
Claude Code.

## Context

- SPEC-445 added Codex accounts by device code (`codex login --device-auth`) or OpenAI API key,
  sealed as an `auth.json`; per-(sandbox, account) Codex homes get the vault's copy before a run
  and write a genuine refresh back after it (`codex_auth_replaces`: same `tokens.account_id`,
  same `id_token` subject, newer `last_refresh`).
- Codex runs start `codex` from the sandbox's `PATH`; the sandbox system layer is a copy of the
  host's, which has no Codex CLI on this instance. Claude Code reaches runs by a read-only bind
  of its npm package (SPEC-421, `wiring/claude_cli.rs`, `agent_cli_binds`).
- `@openai/codex@0.167.0` ships its native build in platform packages
  (`@openai/codex@0.167.0-linux-x64`, 174 MB packed): `package/vendor/<triple>/` holds
  `bin/codex` (static musl), `codex-path/rg`, `codex-resources/` and `codex-package.json`; the
  binary runs from that tree by itself (`codex --version` → `codex-cli 0.167.0`).

## Requirements

| ID | Requirement | Evidence |
| --- | --- | --- |
| IMP-001 | `POST /api/admin/accounts` with `{"provider":"openai","label","auth_json"}` (instance admin) creates the account `ok` at once with the file sealed as its credential; `POST /api/admin/accounts/{id}/import` `{"auth_json"}` replaces an `openai` account's credential and cancels a sign-in running for it. | `tests/accounts.rs` |
| IMP-002 | Only a ChatGPT sign-in is imported: a JSON object with `auth_mode` `chatgpt` and non-empty `tokens.id_token`, `access_token`, `refresh_token` and `account_id`; anything else, a Claude account, or both `api_key` and `auth_json` is 422. The file is never returned, logged or written to the audit (which names `auth_json` only). | `yulius-harness/tests/login.rs`; `tests/accounts.rs`; contracts `Debug` test |
| IMP-003 | An imported sign-in is what a Codex run's home gets, and the tokens Codex refreshes during the run are written back as for a device sign-in. | `tests/codex.rs` |
| IMP-004 | Add subscription → Codex offers "Import auth.json" (paste or file) with a note to sign in with a separate `CODEX_HOME`; a Codex subscription's card imports a new file. The mock serves both endpoints. | `e2e/tests/codex.e2e.ts` |
| CLI-001 | Yulius installs the pinned Codex CLI (`0.167.0`) from npm's registry into `<data>/codex-cli/<version>-<platform>`: the tarball's sha512 must equal the one pinned in the source, only regular files of `vendor/<triple>/` inside the release are unpacked, bounded, `bin/codex` must be an ELF for the platform, the move is atomic, one install at a time, earlier releases go once no process runs them. | `tests/codex_cli.rs` |
| CLI-002 | Every Codex run and Codex sign-in starts the provided `bin/codex` by its absolute path; a contained run binds its directory read-only. Without a provided CLI (an unsupported platform, or an install that failed while the host has `codex`), the host's `codex` is used; an install that failed with no host `codex` fails the start with the reason, never `codex: not found`. | `yulius-harness/tests/launch.rs`; `tests/codex.rs`; `wiring/harness.rs` tests |

## Plan

- [x] A ChatGPT Codex auth.json is validated for import
- [x] Accounts are created or re-signed by importing auth.json
- [x] An imported sign-in reaches the sandbox Codex home and is written back
- [x] Add subscription offers Import auth.json for Codex
- [x] Yulius installs the pinned Codex CLI from npm
- [x] Codex runs and sign-ins use the provided Codex CLI
- [x] SPEC-445, ADR and docs describe import and the provided CLI
- [x] Independent review answered and branch pushed

## Acceptance criteria

- AC-446-1: an administrator pastes or picks the laptop's `auth.json` under Add subscription →
  Codex → Import auth.json and the subscription is connected at once; an API-key file is refused
  with the reason, without echoing it.
- AC-446-2: a Codex task on that account runs with that file in its Codex home, and a refreshed
  file is kept in the account when the run ends.
- AC-446-3: on a host without Codex, a Codex task starts `<data>/codex-cli/0.167.0-linux-x64/bin/codex`
  inside its sandbox.

## Test plan

- Unit: `validate_codex_import`; the pinned release's platform data; the launch plan's program;
  the adapter's fallback to the host's `codex`.
- Integration: accounts API (create by import, import into an account, refusals, audit, a
  cancelled sign-in); a Codex task on an imported account (fake agent) with write-back; the
  installer against a local registry stub (install once, concurrent callers, refused tarballs,
  leftovers and earlier releases removed).
- e2e: the Add subscription import path, pasted and from a file, and the card's import.
- No test calls OpenAI or npm.

## Decisions

- Import is a credential like the API key: no sign-in process, no temporary home.
- `account_id` and all three tokens are required: without `account_id` the write-back rule never
  takes a refresh, and the account would go stale at the first rotated refresh token.
- A separate endpoint `POST /api/admin/accounts/{id}/import` rather than a body on `relogin`,
  which starts a terminal sign-in and answers with its id.
- The Codex CLI is pinned by version and sha512 in the source instead of followed on a channel
  (ADR-0289): the harness is checked against one release.

## Evidence

Run on 2026-11-09/10 on `feat/codex-harness` (from `e3faca10`).

- `cargo fmt --all -- --check`: clean. `cargo clippy -j2 --workspace --all-targets --locked -- -D
  warnings`: clean. `python3 scripts/check_repository.py`: passed. `spec_check`: valid.
- Unit: `cargo test -p yulius-domain -p yulius-contracts -p yulius-harness -p yulius-app -p
  fake-agent` and `cargo test -p yulius --lib`: 1398 passed, 0 failed (`validate_codex_import`,
  the pinned release's data, the launch plan's absolute program, redacted `Debug`, the export
  leaving `codex-cli` out).
- Integration (`cargo test -p yulius --test main -- codex:: codex_cli:: accounts:: agent_hooks::
  claude_gate:: task_delete:: automations:: supervision:: pool_supervision:: subscription_pool
  account_usage:: run_metrics:: restart:: claude_updates:: cockpit:: instance`): 241 passed,
  0 failed, 2 ignored. New: import at creation and into an account (refusals, audit without the
  content, a running sign-in cancelled and its file not taken), an imported sign-in in the run's
  Codex home and its refresh written back, the installer (once, single-flight, refused tarballs,
  umask 077, earlier releases pruned, a failure not retried by every start), the device sign-in
  and a Codex run starting the provided program, a contained run reading it through a real
  bubblewrap bind while the data directory stays hidden, a start without a CLI failing before
  any spawn.
- Live npm (`codex_cli::the_real_pinned_release_installs_from_npm`, `--ignored`): the real
  `@openai/codex@0.167.0-linux-x64` matched the pinned sha512, unpacked, and `bin/codex
  --version` printed `codex-cli 0.167.0` (108 s).
- e2e (`npx e2e run tests/codex.e2e.ts tests/subscriptions.e2e.ts`): 21 passed (import pasted
  and from a file, an API-key file refused with the reason, the card's import; no WCAG
  violations).
- Independent review (fresh read-only session): round 1 REQUEST_CHANGES (2 major, 7 minor), all
  answered test-first in `79e88062` except the host-fallback mismatch and the relogin window,
  recorded in ADR-0289's consequences. Round 2 could not run to its end (the reviewer hit the
  account's API limit); its checks were done by hand: `start_run_if` is the only way into
  `launch_attempt` and installs before the lock.
- Not verified (needs a live Codex account): a real laptop `auth.json` imported and used by a
  run, its refresh by Codex and write-back, and the provided CLI inside a namespace-isolated
  sandbox of this instance.

