//! Full account imports are bounded and throttled independently of outbound retries.
use super::{credentials, providers, reconcile, remote};
use anyhow::Result;
use sqlx::SqlitePool;

pub async fn pull(pool: &SqlitePool, now: i64, force: bool) -> Result<Vec<String>> {
    let mut accounts = credentials::load()?;
    if accounts.is_empty() {
        return Ok(Vec::new());
    }
    let client = providers::client()?;
    let mut messages = Vec::new();
    let mut snapshots = Vec::new();
    for provider in accounts.keys().cloned().collect::<Vec<_>>() {
        let user_id = accounts[&provider].user_id;
        let key = format!("tracker_import_{provider}_{user_id}");
        let previous: Option<i64> =
            sqlx::query_scalar("SELECT synced_at FROM sync_meta WHERE key=?")
                .bind(&key)
                .fetch_optional(pool)
                .await?;
        if !force && previous.is_some_and(|last| last <= now && now - last < 300) {
            continue;
        }
        // Throttle failed imports too; explicit retries bypass this cooldown.
        sqlx::query("INSERT INTO sync_meta(key,synced_at) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET synced_at=excluded.synced_at")
            .bind(&key).bind(now).execute(pool).await?;
        let refresh = if provider == "mal" {
            providers::refresh(
                &client,
                accounts
                    .get_mut(&provider)
                    .expect("Connected account exists"),
                now,
            )
            .await
        } else {
            Ok(())
        };
        // Commit rotating credentials before any later request or database import can fail.
        credentials::save(&accounts)?;
        let result = match refresh {
            Ok(()) => remote::fetch(&client, pool, &provider, &accounts[&provider], now).await,
            Err(error) => Err(error),
        };
        match result {
            Ok(list) => match reconcile::import(pool, &list.entries).await {
                Ok(changed) => {
                    messages.push(format!(
                        "{provider}: imported progress for {changed} titles"
                    ));
                    if list.unmapped > 0 {
                        messages.push(format!(
                            "{provider}: skipped {} titles without an AniList mapping",
                            list.unmapped
                        ));
                    }
                    snapshots.push((provider, user_id, key, list.entries));
                }
                Err(error) => messages.push(format!(
                    "{provider}: import failed: {error}; local progress retained"
                )),
            },
            Err(error) => messages.push(format!(
                "{provider}: import failed: {error}; local progress retained"
            )),
        }
    }
    for (provider, user_id, key, entries) in snapshots {
        let queued = reconcile::catch_up(pool, &provider, user_id, &entries).await?;
        if queued > 0 {
            messages.push(format!(
                "{provider}: queued {queued} titles to match local progress"
            ));
        }
        sqlx::query("INSERT INTO sync_meta(key,synced_at) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET synced_at=excluded.synced_at")
            .bind(key).bind(now).execute(pool).await?;
    }
    Ok(messages)
}
