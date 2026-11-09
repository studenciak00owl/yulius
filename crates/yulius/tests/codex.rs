//! Codex as a second harness with real processes (SPEC-445): supervisor → detached
//! `yulius ptyhost` → fake agent writing a Codex CLI 0.167 shaped rollout. A Codex task runs on a
//! Codex account in its own Codex home, its rollout is found and watched, its usage reaches its
//! account, it moves to another Codex account at a usage limit by a native resume, a restart
//! reattaches it, and the credentials Codex refreshed are kept. No test calls OpenAI.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::supervision::{
    build_full, runtime, screen_contains, seed_records, seed_repo, task, wait_for,
};
use crate::support;
use crate::support::TestResult;
use crate::support::agents::FakeAgentOptions;
use yulius_app::context::App;
use yulius_app::lifecycle::NewTask;
use yulius_app::model::{AccountRecord, AccountStatus, LaunchKind, TaskOrigin};
use yulius_domain::harness::{Harness, Provider};
use yulius_domain::ids::{AccountId, SandboxId, TaskId, UserId};
use yulius_domain::task::TaskStatus;
use yulius_domain::time::Timestamp;
use yulius_ptyhost::test_support::locate_fake_agent;

/// A synthetic Codex `auth.json` refreshed at `last_refresh`.
fn codex_auth(last_refresh: &str) -> String {
    format!(
        r#"{{"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{{"id_token":"h.e30.s","access_token":"SYNTHETIC-access-{last_refresh}","refresh_token":"SYNTHETIC-refresh","account_id":"acc-synthetic"}},"last_refresh":"{last_refresh}"}}"#
    )
}

async fn add_codex_account(
    app: &App,
    id: &str,
    auth: &str,
) -> Result<AccountId, Box<dyn std::error::Error>> {
    let account = AccountId::parse(id)?;
    app.deps
        .store
        .insert_account(&AccountRecord {
            id: account.clone(),
            provider: Provider::OpenAi,
            label: format!("Codex {id}"),
            credential: Some(app.deps.crypto.seal(auth.as_bytes())?),
            status: AccountStatus::Ok,
            usage_json: None,
            last_error: None,
            created_at: Timestamp(1),
            updated_at: Timestamp(1),
            email: None,
            email_checked_at: None,
        })
        .await?;
    Ok(account)
}

async fn new_codex_task(
    app: &Arc<App>,
    sandbox: &SandboxId,
    user: &UserId,
    title: &str,
) -> Result<TaskId, Box<dyn std::error::Error>> {
    let record = app
        .create_task_internal(NewTask {
            sandbox: sandbox.clone(),
            created_by: user.clone(),
            prompt: format!("{title}: answer with an echo"),
            harness: Harness::Codex,
            title: Some(title.into()),
            workflow_id: None,
            steps: Vec::new(),
            model: None,
            effort: None,
            origin: TaskOrigin::Ui,
            automation_run: None,
            start: true,
            mode: yulius_app::model::TaskMode::Prompt,
            supertask: false,
        })
        .await?;
    Ok(record.id)
}

async fn stored_credential(
    app: &App,
    account: &AccountId,
) -> Result<String, Box<dyn std::error::Error>> {
    let sealed = app
        .deps
        .store
        .account(account)
        .await?
        .ok_or("account")?
        .credential
        .ok_or("credential")?;
    Ok(String::from_utf8(app.deps.crypto.open(&sealed)?)?)
}

/// An RFC 3339 time `ms_ago` milliseconds before now (a refresh Codex just made).
fn ago(ms_ago: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0));
    yulius_app::auth::rfc3339(Timestamp(now - ms_ago))
}

/// The output tokens stored for `attempt`.
fn saved_output(data: &Path, attempt: &str) -> Option<i64> {
    let db = rusqlite::Connection::open(data.join("yulius.db")).ok()?;
    db.query_row(
        "SELECT output_tokens FROM metrics WHERE attempt_id = ?1",
        [attempt],
        |row| row.get(0),
    )
    .ok()
    .flatten()
}

fn temp() -> Result<(tempfile::TempDir, PathBuf), Box<dyn std::error::Error>> {
    // A short base: run sockets live at <data>/runs/<uuid>/pty.sock (Unix path limit).
    let dir = tempfile::Builder::new().prefix("yl").tempdir_in("/tmp")?;
    let data = dir.path().canonicalize()?;
    Ok((dir, data))
}

/// SPEC-445 CDX-002, CDX-003, CDX-004, CDX-006, AC-445-1/3/5: a Codex task runs Codex on a Codex
/// account (never the Claude one) in `<sandbox>/codex/<account>`, whose `auth.json`, `config.toml`
/// and `hooks.json` are private and whose `sessions` is the sandbox's; its session is learnt from
/// the rollout; its turns, usage and rate limits are read; a restart reattaches the same process
/// and reads the rollout again; and when the run ends, the credentials Codex refreshed meanwhile
/// replace the stored ones.
#[test]
fn a_codex_task_runs_in_its_home_survives_a_restart_and_keeps_refreshed_credentials() -> TestResult
{
    let fake = locate_fake_agent().ok_or("fake-agent is not built (cargo build -p fake-agent)")?;
    let (_dir, data) = temp()?;
    let agent = FakeAgentOptions {
        transcript: true,
        ..FakeAgentOptions::default()
    };
    let first = runtime()?;
    let (task_id, account, child_pid, home) = first.block_on(async {
        let app = build_full(&data, Some(&fake), agent.clone(), 8)?;
        let (sandbox, user) = seed_records(&app).await?;
        let account =
            add_codex_account(&app, "a-codex", &codex_auth("2026-11-01T10:00:00Z")).await?;
        seed_repo(&data, &sandbox)?;
        let id = new_codex_task(&app, &sandbox, &user, "Codex echo").await?;
        wait_for("the fake agent banner", Duration::from_secs(30), || {
            screen_contains(&app, &id, "FAKE-AGENT READY")
        })
        .await?;
        let attempt = app
            .deps
            .store
            .live_attempt(&id)
            .await?
            .ok_or("no live attempt")?;
        assert_eq!(attempt.harness, Harness::Codex);
        assert_eq!(
            attempt.account_id.as_ref(),
            Some(&account),
            "a Codex account"
        );
        assert_eq!(attempt.provider_session_id, None, "Codex names its session");

        let sandbox_dir = data.join("sandboxes").join(sandbox.as_str());
        let home = sandbox_dir.join("codex").join(account.as_str());
        let auth = std::fs::read_to_string(home.join("auth.json"))?;
        assert_eq!(auth, codex_auth("2026-11-01T10:00:00Z"));
        for file in ["auth.json", "config.toml", "hooks.json"] {
            let mode = std::fs::metadata(home.join(file))?.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{file}");
        }
        let worktree = task(&app, &id).await?.worktree_path.ok_or("worktree")?;
        let config = std::fs::read_to_string(home.join("config.toml"))?;
        assert!(
            config.contains(&format!(
                "[projects.\"{worktree}\"]\ntrust_level = \"trusted\""
            )),
            "{config}"
        );
        assert_eq!(
            std::fs::read_link(home.join("sessions"))?,
            PathBuf::from("../sessions")
        );
        let hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(home.join("hooks.json"))?)?;
        assert!(
            hooks["hooks"]["Stop"][0]["hooks"][0]["command"]
                .as_str()
                .is_some_and(|c| c.ends_with("hook codex Stop")),
            "{hooks}"
        );

        app.send_input(&id, b"hello codex\r").await?;
        wait_for("the echo", Duration::from_secs(10), || {
            screen_contains(&app, &id, "ECHO: hello codex")
        })
        .await?;
        // The rollout is in the sandbox's shared sessions, found by the id Codex gave it.
        let session = attempt.id.as_str().to_owned();
        wait_for(
            "the session learnt from the rollout",
            Duration::from_secs(30),
            || async {
                app.deps
                    .store
                    .attempt(&attempt.id)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|a| a.provider_session_id)
                    .as_deref()
                    == Some(session.as_str())
            },
        )
        .await?;
        // The lineage records it in the same watch round, right after the attempt.
        wait_for(
            "the session in the lineage",
            Duration::from_secs(10),
            || async {
                app.deps
                    .store
                    .provider_sessions(&id)
                    .await
                    .unwrap_or_default()
                    .iter()
                    .any(|l| l.session_id == session && l.harness == Harness::Codex)
            },
        )
        .await?;
        assert!(
            sandbox_dir
                .join("codex/sessions/2026/11/09")
                .join(format!("rollout-2026-11-09T00-00-00-{session}.jsonl"))
                .is_file()
        );
        // Its usage and the account's rate limits come from the rollout.
        wait_for("the run's tokens", Duration::from_secs(30), || async {
            saved_output(&data, attempt.id.as_str()).is_some_and(|n| n > 0)
        })
        .await?;
        wait_for("the account's window", Duration::from_secs(30), || async {
            app.deps
                .store
                .account(&account)
                .await
                .ok()
                .flatten()
                .and_then(|a| a.usage_json)
                .is_some_and(|json| {
                    yulius_app::usage_json::from_json(&json)
                        .primary
                        .and_then(|w| w.used_percent)
                        == Some(10.0)
                })
        })
        .await?;
        Ok::<_, Box<dyn std::error::Error>>((id, account, attempt.child_pid, home))
    })?;
    // The server "crashes"; the run goes on in its host.
    first.shutdown_timeout(Duration::from_secs(5));

    let second = runtime()?;
    second.block_on(async {
        let app = build_full(&data, Some(&fake), agent, 8)?;
        app.reconcile().await?;
        let attempt = app
            .deps
            .store
            .live_attempt(&task_id)
            .await?
            .ok_or("attempt lost on restart")?;
        assert_eq!(attempt.child_pid, child_pid, "same process, not respawned");
        app.send_input(&task_id, b"again\r").await?;
        wait_for("the echo after restart", Duration::from_secs(10), || {
            screen_contains(&app, &task_id, "ECHO: again")
        })
        .await?;
        // Codex refreshed its tokens during the run: the newer file is kept when the run ends,
        // and the account, limited meanwhile, keeps its state.
        let refreshed = codex_auth(&ago(60_000));
        std::fs::write(home.join("auth.json"), &refreshed)?;
        app.deps
            .store
            .set_account_state(&account, AccountStatus::Limited, None, None, app.now())
            .await?;
        app.send_input(&task_id, b"/exit\r").await?;
        wait_for("the run's end", Duration::from_secs(30), || async {
            task(&app, &task_id)
                .await
                .is_ok_and(|t| !t.status.is_live())
        })
        .await?;
        wait_for(
            "the written-back credentials",
            Duration::from_secs(10),
            || async {
                stored_credential(&app, &account)
                    .await
                    .is_ok_and(|stored| stored == refreshed)
            },
        )
        .await?;
        let stored = app.deps.store.account(&account).await?.ok_or("account")?;
        assert_eq!(
            stored.status,
            AccountStatus::Limited,
            "the account keeps its state"
        );
        // No run of the account is left in the sandbox: its credential does not stay there.
        assert!(
            !home.join("auth.json").exists(),
            "auth.json left in the sandbox"
        );
        assert!(home.join("config.toml").exists());
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    second.shutdown_timeout(Duration::from_secs(5));
    Ok(())
}

/// SPEC-445 CDX-006: the Codex home's `auth.json` is the agent's to write, so only a genuine
/// refresh goes back into the vault. A newer one another run of the account left in the sandbox
/// is taken at the next launch instead of being overwritten with the vault's older copy; one the
/// agent forged (another ChatGPT account) is never taken.
#[test]
fn only_a_genuine_codex_refresh_reaches_the_vault() -> TestResult {
    let fake = locate_fake_agent().ok_or("fake-agent is not built (cargo build -p fake-agent)")?;
    let (_dir, data) = temp()?;
    let rt = runtime()?;
    rt.block_on(async {
        let app = build_full(&data, Some(&fake), FakeAgentOptions::default(), 8)?;
        let (sandbox, user) = seed_records(&app).await?;
        let account =
            add_codex_account(&app, "a-codex", &codex_auth("2026-11-01T10:00:00Z")).await?;
        seed_repo(&data, &sandbox)?;
        // Another run of the account refreshed its tokens and is still going: its file is newer.
        let home = data
            .join("sandboxes")
            .join(sandbox.as_str())
            .join("codex")
            .join(account.as_str());
        std::fs::create_dir_all(&home)?;
        let left = codex_auth(&ago(120_000));
        std::fs::write(home.join("auth.json"), &left)?;
        let id = new_codex_task(&app, &sandbox, &user, "Codex refresh").await?;
        wait_for("the banner", Duration::from_secs(30), || {
            screen_contains(&app, &id, "FAKE-AGENT READY")
        })
        .await?;
        assert_eq!(std::fs::read_to_string(home.join("auth.json"))?, left);
        assert_eq!(stored_credential(&app, &account).await?, left);
        // The agent writes a sign-in of its own: it is not taken when the run ends.
        let forged = codex_auth(&ago(1_000)).replace("acc-synthetic", "acc-attacker");
        std::fs::write(home.join("auth.json"), &forged)?;
        app.send_input(&id, b"/exit\r").await?;
        wait_for("the run's end", Duration::from_secs(30), || async {
            task(&app, &id).await.is_ok_and(|t| !t.status.is_live())
        })
        .await?;
        assert_eq!(stored_credential(&app, &account).await?, left);
        // A file that did not reach the vault is not removed: were it a genuine refresh that
        // could not be stored, it is the only copy of the rotated tokens.
        assert_eq!(std::fs::read_to_string(home.join("auth.json"))?, forged);
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    rt.shutdown_timeout(Duration::from_secs(5));
    Ok(())
}

/// SPEC-445 CDX-009, AC-445-2: a Codex task at its usage limit moves to another Codex account by
/// a native resume of the same session, never to the Claude account, and the limited account is
/// marked; Codex does not go on by itself, so the resume asks it to.
#[test]
fn a_codex_task_at_its_limit_moves_to_another_codex_account() -> TestResult {
    let fake = locate_fake_agent().ok_or("fake-agent is not built (cargo build -p fake-agent)")?;
    let (_dir, data) = temp()?;
    let rt = runtime()?;
    rt.block_on(async {
        let agent = FakeAgentOptions {
            transcript: true,
            limit_after: Some(1),
            ..FakeAgentOptions::default()
        };
        let app = build_full(&data, Some(&fake), agent, 8)?;
        let (sandbox, user) = seed_records(&app).await?;
        let auth = codex_auth("2026-11-01T10:00:00Z");
        let one = add_codex_account(&app, "a-codex-1", &auth).await?;
        let two = add_codex_account(&app, "a-codex-2", &auth).await?;
        seed_repo(&data, &sandbox)?;
        let id = new_codex_task(&app, &sandbox, &user, "Codex rotation").await?;
        wait_for("the banner", Duration::from_secs(30), || {
            screen_contains(&app, &id, "FAKE-AGENT READY")
        })
        .await?;
        app.send_input(&id, b"hello\r").await?;
        wait_for(
            "the switch to another account",
            Duration::from_secs(90),
            || async {
                let attempts = app.deps.store.attempts(&id).await.unwrap_or_default();
                attempts.len() >= 2 && attempts.last().is_some_and(|a| !a.state.is_terminal())
            },
        )
        .await?;
        let attempts = app.deps.store.attempts(&id).await?;
        let (first, second) = (&attempts[0], &attempts[1]);
        let accounts = [Some(one.clone()), Some(two.clone())];
        assert!(accounts.contains(&first.account_id), "{first:?}");
        assert!(accounts.contains(&second.account_id), "{second:?}");
        assert_ne!(first.account_id, second.account_id, "another Codex account");
        assert_eq!(second.harness, Harness::Codex, "the task keeps its harness");
        assert_eq!(second.launch_kind, LaunchKind::Resume);
        assert!(first.provider_session_id.is_some());
        assert_eq!(
            first.provider_session_id, second.provider_session_id,
            "same session"
        );
        let launch = std::fs::read_to_string(Path::new(&second.run_dir).join("fake-launch.txt"))?;
        assert!(
            launch.contains("Resume {") && launch.contains("continue_interrupted: true"),
            "{launch}"
        );
        wait_for("the resumed run", Duration::from_secs(30), || async {
            task(&app, &id)
                .await
                .is_ok_and(|t| t.status == TaskStatus::Running)
        })
        .await?;
        let limited = app
            .deps
            .store
            .account(first.account_id.as_ref().ok_or("first account")?)
            .await?
            .ok_or("the first account")?;
        assert_eq!(limited.status, AccountStatus::Limited);
        let claude = app
            .deps
            .store
            .account(&AccountId::parse("a-claude")?)
            .await?
            .ok_or("claude")?;
        assert_eq!(
            claude.status,
            AccountStatus::Ok,
            "the Claude account is untouched"
        );
        support::stop_tasks(&app, &[&id]).await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    rt.shutdown_timeout(Duration::from_secs(5));
    Ok(())
}

/// SPEC-445 CDX-009: with no Codex account left with capacity, a Codex task waits instead of
/// running on the Claude account.
#[test]
fn a_codex_task_waits_while_no_codex_account_has_capacity() -> TestResult {
    let fake = locate_fake_agent().ok_or("fake-agent is not built (cargo build -p fake-agent)")?;
    let (_dir, data) = temp()?;
    let rt = runtime()?;
    rt.block_on(async {
        let app = build_full(&data, Some(&fake), FakeAgentOptions::default(), 8)?;
        let (sandbox, user) = seed_records(&app).await?;
        let account =
            add_codex_account(&app, "a-codex", &codex_auth("2026-11-01T10:00:00Z")).await?;
        let full = yulius_domain::usage::AccountUsage {
            primary: Some(yulius_domain::usage::UsageWindow {
                used_percent: Some(100.0),
                window_minutes: Some(300),
                resets_at: Some(Timestamp(4_000_000_000_000)),
            }),
            limit_reached: true,
            limit_resets_at: Some(Timestamp(4_000_000_000_000)),
            ..yulius_domain::usage::AccountUsage::default()
        };
        app.deps
            .store
            .set_account_state(
                &account,
                AccountStatus::Limited,
                Some(&yulius_app::usage_json::to_json(&full)),
                None,
                app.now(),
            )
            .await?;
        seed_repo(&data, &sandbox)?;
        let id = new_codex_task(&app, &sandbox, &user, "Codex waits").await?;
        wait_for("the pause", Duration::from_secs(30), || async {
            task(&app, &id)
                .await
                .is_ok_and(|t| t.status == TaskStatus::WaitingCapacity)
        })
        .await?;
        assert!(
            app.deps.store.attempts(&id).await?.is_empty(),
            "no run on the Claude account"
        );
        Ok::<_, Box<dyn std::error::Error>>(())
    })?;
    rt.shutdown_timeout(Duration::from_secs(5));
    Ok(())
}
