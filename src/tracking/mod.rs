//! Opt-in account tracking. A launch records progress; it does not verify viewing completion.
mod credentials;
mod oauth;
mod providers;
use crate::db::cache::Anime;
use anyhow::{bail, Result};
use sqlx::SqlitePool;
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub struct AccountSummary {
    pub provider: String,
    pub name: String,
    pub connected: bool,
    pub error: Option<String>,
}
pub fn account_summary() -> Vec<AccountSummary> {
    let (accounts, error) = match credentials::load() {
        Ok(accounts) => (accounts, None),
        Err(_) => (
            credentials::Accounts::new(),
            Some("Cannot read account credentials; check accounts.json".to_string()),
        ),
    };
    ["anilist", "mal"]
        .iter()
        .map(|provider| AccountSummary {
            provider: (*provider).into(),
            name: accounts
                .get(*provider)
                .map(|a| a.username.clone())
                .unwrap_or_default(),
            connected: accounts.contains_key(*provider),
            error: error.clone(),
        })
        .collect()
}
async fn tables(pool: &SqlitePool) -> Result<()> {
    sqlx::query("CREATE TABLE IF NOT EXISTS tracker_jobs(provider TEXT NOT NULL,anime_id INTEGER NOT NULL,progress INTEGER NOT NULL,user_id INTEGER NOT NULL,error TEXT,PRIMARY KEY(provider,anime_id))").execute(pool).await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS tracker_ids(anime_id INTEGER PRIMARY KEY,mal_id INTEGER NOT NULL)").execute(pool).await?;
    Ok(())
}
async fn queue(
    pool: &SqlitePool,
    provider: &str,
    anime_id: i64,
    progress: i64,
    user_id: i64,
) -> Result<()> {
    sqlx::query("INSERT INTO tracker_jobs(provider,anime_id,progress,user_id) VALUES(?,?,?,?) ON CONFLICT(provider,anime_id) DO UPDATE SET progress=CASE WHEN user_id=excluded.user_id THEN MAX(progress,excluded.progress) ELSE excluded.progress END,user_id=excluded.user_id,error=NULL").bind(provider).bind(anime_id).bind(progress).bind(user_id).execute(pool).await?;
    Ok(())
}
pub async fn queue_progress(pool: &SqlitePool, anime: &Anime, episode: u32) -> Vec<String> {
    let result: Result<()> = async {
        if episode == 0 {
            return Ok(());
        }
        // Atomic credentials reads and identity-tagged rows keep launch persistence independent
        // of remote sync locks. A disconnected/replaced identity is rejected by the drain.
        let accounts = credentials::load()?;
        if accounts.is_empty() {
            return Ok(());
        }
        tables(pool).await?;
        for (provider, account) in accounts {
            queue(
                pool,
                &provider,
                anime.id,
                i64::from(episode),
                account.user_id,
            )
            .await?;
        }
        Ok(())
    }
    .await;
    if result.is_ok() {
        Vec::new()
    } else {
        vec!["Account tracking could not save progress; check credentials and database".into()]
    }
}
pub async fn retry_pending(pool: &SqlitePool, now: i64) -> Vec<String> {
    let _guard = SERIAL.lock().await;
    let result = async {
        let _process_guard = credentials::lock().await?;
        sync_pending(pool, now).await
    }
    .await;
    match result {
        Ok(messages) => messages,
        Err(_) => vec!["Account tracking retry failed; check credentials and database".into()],
    }
}
async fn sync_pending(pool: &SqlitePool, now: i64) -> Result<Vec<String>> {
    tables(pool).await?;
    let mut accounts = credentials::load()?;
    let client = providers::client()?;
    let jobs: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT provider,anime_id,progress,user_id FROM tracker_jobs ORDER BY provider,anime_id",
    )
    .fetch_all(pool)
    .await?;
    let mut messages = Vec::new();
    for (provider, id, progress, user_id) in jobs {
        let Some(account) = accounts.get_mut(&provider) else {
            continue;
        };
        if account.user_id != user_id {
            sqlx::query("DELETE FROM tracker_jobs WHERE provider=? AND anime_id=? AND user_id=?")
                .bind(&provider)
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await?;
            continue;
        }
        let result = async {
            if provider == "mal" {
                providers::refresh(&client, account, now).await?;
            }
            let anime = crate::db::cache::get_anime(pool, id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Anime metadata missing"))?;
            providers::sync(&client, pool, &provider, account, &anime, progress).await
        }
        .await;
        // Persist rotating refresh tokens even if the following progress request fails.
        credentials::save(&accounts)?;
        match result {
            Ok(()) => {
                sqlx::query(
                    "DELETE FROM tracker_jobs WHERE provider=? AND anime_id=? AND progress<=? AND user_id=?",
                )
                .bind(&provider)
                .bind(id)
                .bind(progress)
                .bind(user_id)
                .execute(pool)
                .await?;
                messages.push(format!("{provider}: progress synced through E{progress}"));
            }
            Err(error) => {
                let message = error.to_string();
                sqlx::query(
                    "UPDATE tracker_jobs SET error=? WHERE provider=? AND anime_id=? AND user_id=?",
                )
                .bind(&message)
                .bind(&provider)
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await?;
                messages.push(format!("{provider}: {message}; progress queued for retry"));
            }
        }
    }
    Ok(messages)
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub async fn handle_cli(args: &[String], pool: &SqlitePool) -> Result<bool> {
    if args.first().map(String::as_str) != Some("accounts") {
        return Ok(false);
    }
    match args.get(1).map(String::as_str).unwrap_or("status") {
        "status" => {
            for account in account_summary() {
                println!(
                    "{}: {}",
                    account.provider,
                    if let Some(error) = account.error.as_deref() {
                        error
                    } else if account.connected {
                        account.name.as_str()
                    } else {
                        "not connected"
                    }
                );
            }
        }
        "connect" => {
            let provider = args.get(2).map(String::as_str).unwrap_or("");
            if !["anilist", "mal"].contains(&provider) {
                bail!("Usage: ani-tui accounts connect anilist|mal CLIENT_ID");
            }
            let Some(client_id) = args.get(3) else {
                if provider == "anilist" {
                    println!("Register at https://anilist.co/settings/developer\nSet redirect URL: https://anilist.co/api/v2/oauth/pin");
                } else {
                    println!("Register a native app at https://myanimelist.net/apiconfig\nSet redirect URL: http://127.0.0.1:8766/callback\nDo not share a client secret.");
                }
                println!("Then run: ani-tui accounts connect {provider} CLIENT_ID");
                return Ok(true);
            };
            let _guard = SERIAL.lock().await;
            let account = oauth::connect(provider, client_id, now()).await?;
            let _process_guard = credentials::lock().await?;
            let username = account.username.clone();
            let mut accounts = credentials::load()?;
            if accounts
                .get(provider)
                .is_none_or(|old| old.user_id != account.user_id)
            {
                tables(pool).await?;
                sqlx::query("DELETE FROM tracker_jobs WHERE provider=?")
                    .bind(provider)
                    .execute(pool)
                    .await?;
            }
            accounts.insert(provider.into(), account);
            credentials::save(&accounts)?;
            println!("Connected {provider} as {username}. Episode launches will update progress.");
        }
        "disconnect" => {
            let provider = args.get(2).map(String::as_str).unwrap_or("");
            if !["anilist", "mal"].contains(&provider) {
                bail!("Usage: ani-tui accounts disconnect anilist|mal");
            }
            let _guard = SERIAL.lock().await;
            let _process_guard = credentials::lock().await?;
            let mut accounts = credentials::load()?;
            accounts.remove(provider);
            credentials::save(&accounts)?;
            tables(pool).await?;
            sqlx::query("DELETE FROM tracker_jobs WHERE provider=?")
                .bind(provider)
                .execute(pool)
                .await?;
            println!("Disconnected {provider}; pending updates removed. Revoke authorization on the provider website if desired.");
        }
        "retry" => {
            for message in retry_pending(pool, now()).await {
                println!("{message}");
            }
        }
        _ => bail!("Usage: ani-tui accounts status|connect|disconnect|retry"),
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn pending_progress_coalesces_without_decreasing() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        tables(&pool).await.unwrap();
        queue(&pool, "anilist", 1, 5, 11).await.unwrap();
        queue(&pool, "anilist", 1, 2, 11).await.unwrap();
        queue(&pool, "mal", 1, 3, 22).await.unwrap();
        let entries: Vec<(String, i64)> =
            sqlx::query_as("SELECT provider,progress FROM tracker_jobs ORDER BY provider")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(entries, vec![("anilist".into(), 5), ("mal".into(), 3)]);
        queue(&pool, "anilist", 1, 7, 11).await.unwrap();
        let progress: i64 =
            sqlx::query_scalar("SELECT progress FROM tracker_jobs WHERE provider='anilist'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(progress, 7);
    }

    #[tokio::test]
    async fn queue_persists_while_sync_lock_is_held_and_isolates_identity() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        tables(&pool).await.unwrap();
        let _sync = SERIAL.lock().await;
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            queue(&pool, "anilist", 1, 9, 11),
        )
        .await
        .unwrap()
        .unwrap();
        queue(&pool, "anilist", 1, 2, 12).await.unwrap();
        let row: (i64, i64) = sqlx::query_as("SELECT progress,user_id FROM tracker_jobs")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row, (2, 12));
        sqlx::query("DELETE FROM tracker_jobs WHERE provider=? AND anime_id=? AND progress<=? AND user_id=?").bind("anilist").bind(1).bind(9).bind(11).execute(&pool).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tracker_jobs")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
    }
}
