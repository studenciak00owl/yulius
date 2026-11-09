# 0289. Yulius provides the pinned Codex CLI, and a Codex sign-in can be imported

## Status

Accepted for the fork `studenciak00owl/yulius` only. Specified in
[SPEC-446](../../.ai/specs/SPEC-446-codex-import-auth-json-codex-cli-in-sand.md) (also SPEC-445
CDX-011, CDX-012); fork task #4. Extends [ADR-0288](0288-fork-codex-returns-as-a-second-harness.md).

## Context

- A Codex task ran `codex` from its sandbox's `PATH`. Sandboxes copy the host system, and this
  instance's host has no Codex CLI, so a Codex task failed with `codex: not found` unless someone
  ran `sudo npm i -g @openai/codex@0.167.0` inside that sandbox. Claude Code does not have this
  problem: Yulius binds the host's npm package and keeps a current copy in its data directory
  ([ADR-0266](0266-yulius-keeps-claude-code-current-in-its-data-directory.md)).
- The harness was checked against Codex CLI 0.167.0 (ADR-0288); its argv, config keys, hook and
  rollout formats change between releases.
- `@openai/codex@0.167.0` ships native builds as platform packages
  (`@openai/codex@0.167.0-linux-x64`): `package/vendor/<triple>/` with a static `bin/codex`,
  `codex-path/rg`, `codex-resources/` and `codex-package.json`, which the binary reads by itself.
- The owner signs Codex in on his laptop (`CODEX_HOME=~/codex-yulius codex login`, ChatGPT in the
  browser) and does not want to depend on the ChatGPT setting the device-code sign-in needs.

## Decision

- Yulius installs one pinned Codex CLI release, `0.167.0`, into
  `<data>/codex-cli/<version>-<platform>`: the platform tarball from npm's registry, accepted only
  when its sha512 equals the one written in the source (`yulius_domain::agent_cli::codex`), and
  only the regular files of its `vendor/<triple>/` tree, bounded, unpacked under a temporary name
  and renamed into place. No `npm`, no package script, nothing executed on the host. A bump of
  the pin is a code change that checks the harness against the new release.
- The install runs on the first need: at start when Codex accounts exist, else at the first Codex
  run or sign-in, outside the launch and sign-in locks; one at a time; a failure answers for a
  minute before the next try. Earlier pins go once no process runs them.
- Codex runs and Codex sign-ins start `<dir>/bin/codex` by its absolute path; a contained run
  binds `<dir>` read-only at the same path. While the release cannot be installed, a host that
  has its own `codex` uses it; without one the start fails with the reason.
- A Codex account can also take the `auth.json` of a ChatGPT sign-in made elsewhere:
  `POST /api/admin/accounts` with `auth_json`, or `POST /api/admin/accounts/{id}/import` for an
  existing account. Only a full ChatGPT sign-in is taken (`auth_mode` `chatgpt`, `id_token`,
  `access_token`, `refresh_token`, `account_id`), so its refreshes are written back by ADR-0288's
  rule. It is sealed like the device sign-in's file, never returned, logged or audited.

## Alternatives

- **Install with `npm i -g` in each sandbox on first use**: needs Node and root in the sandbox,
  writes the agent's own system layer, runs package scripts, and repeats per sandbox.
- **Bind the host's npm install, as for Claude Code**: needs a person to install Codex on the host
  and keep it at the checked release; the host has none today.
- **Follow npm's `latest` like Claude Code's channel**: a release the harness was not checked
  against would reach every Codex run.
- **Trust the registry's `integrity`**: the registry's answer would decide what runs; the pin
  makes the release reproducible and needs no document request.
- **Import through `relogin` with a body**: `relogin` starts a terminal sign-in and answers with
  its id; a separate endpoint keeps both contracts plain.

## Consequences

- About 460 MB in the data directory per installed release, and a 174 MB download on the first
  Codex use of an instance (or at start when Codex accounts exist).
- Codex releases reach runs only through a pin bump in this repository.
- An imported `auth.json` from a laptop that keeps using the same `CODEX_HOME` shares one refresh
  token with Yulius: Codex rotates refresh tokens, so one side's copy goes stale. The UI asks for a
  separate `CODEX_HOME`.
- Other architectures than Linux x86-64 and arm64 get no provided CLI and use the host's `codex`.
