//! Merge remote progress without changing existing local watch timestamps.
use super::remote::RemoteEntry;
use anyhow::{bail, Result};
use sqlx::SqlitePool;
use std::collections::HashMap;

pub async fn import(pool: &SqlitePool, entries: &[RemoteEntry]) -> Result<usize> {
    let total: i64 = entries.iter().map(|entry| entry.progress).sum();
    if total > 1_000_000
        || entries
            .iter()
            .any(|entry| !(0..=10_000).contains(&entry.progress))
    {
        bail!("Remote progress exceeds supported import limits");
    }
    // Metadata must exist before history foreign keys can reference it. Preserve local dub hints.
    for entry in entries.iter().filter(|entry| entry.progress > 0) {
        let mut anime = entry.anime.clone();
        if let Some(existing) = crate::db::cache::get_anime(pool, anime.id).await? {
            anime.has_dub = existing.has_dub;
        }
        crate::db::cache::upsert_anime(pool, &anime).await?;
    }
    let mut tx = pool.begin().await?;
    let mut changed = 0;
    for entry in entries.iter().filter(|entry| entry.progress > 0) {
        // Remote trackers expose a contiguous count, not individual episode events.
        sqlx::query("WITH RECURSIVE episodes(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM episodes WHERE n<?) INSERT OR IGNORE INTO history(anime_id,episode,watched_at) SELECT ?,n,? FROM episodes")
            .bind(entry.progress).bind(entry.anime.id).bind(entry.updated_at).execute(&mut *tx).await?;
        changed += sqlx::query("INSERT INTO continue_watching(anime_id,last_episode,last_watched) VALUES(?,?,?) ON CONFLICT(anime_id) DO UPDATE SET last_episode=excluded.last_episode,last_watched=MAX(continue_watching.last_watched,excluded.last_watched) WHERE excluded.last_episode>continue_watching.last_episode")
            .bind(entry.anime.id).bind(entry.progress).bind(entry.updated_at).execute(&mut *tx).await?.rows_affected() as usize;
    }
    tx.commit().await?;
    Ok(changed)
}

/// Compare complete provider snapshots with the highest confirmed local progress.
pub async fn catch_up(
    pool: &SqlitePool,
    provider: &str,
    user_id: i64,
    entries: &[RemoteEntry],
) -> Result<usize> {
    let remote: HashMap<i64, i64> = entries
        .iter()
        .map(|entry| (entry.anime.id, entry.progress))
        .collect();
    let local: Vec<(i64, i64)> = sqlx::query_as("SELECT anime_id,MAX(progress) FROM (SELECT anime_id,MAX(episode) AS progress FROM history GROUP BY anime_id UNION ALL SELECT anime_id,last_episode FROM continue_watching) GROUP BY anime_id")
        .fetch_all(pool).await?;
    let mut queued = 0;
    for (id, progress) in local {
        if progress > remote.get(&id).copied().unwrap_or(0) && (1..=10_000).contains(&progress) {
            super::queue(pool, provider, id, progress, user_id).await?;
            queued += 1;
        }
    }
    Ok(queued)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn entry(id: i64, progress: i64, updated_at: i64) -> RemoteEntry {
        RemoteEntry { progress, updated_at, anime: serde_json::from_value(json!({"id":id,"title_english":null,"title_romaji":"Fixture","title_native":null,"description":null,"episodes":12,"status":"FINISHED","season":null,"season_year":null,"score":80,"format":"TV","genres":"[]","cover_url":null,"cover_blob":null,"has_dub":0,"updated_at":updated_at})).unwrap() }
    }
    #[tokio::test]
    async fn import_populates_progress_preserves_local_events_and_is_idempotent() {
        let pool = crate::db::init(":memory:").await.unwrap();
        super::super::tables(&pool).await.unwrap();
        assert_eq!(import(&pool, &[entry(1, 3, 100)]).await.unwrap(), 1);
        assert_eq!(
            crate::db::user::get_watched_episodes(&pool, 1)
                .await
                .unwrap(),
            vec![1, 2, 3]
        );
        crate::db::user::record_watched(&pool, 1, 5, 200)
            .await
            .unwrap();
        assert_eq!(import(&pool, &[entry(1, 4, 300)]).await.unwrap(), 0);
        let resume = crate::db::user::get_continue_entry(&pool, 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!((resume.last_episode, resume.last_watched), (5, 200));
        assert_eq!(import(&pool, &[entry(1, 7, 150)]).await.unwrap(), 1);
        let resume = crate::db::user::get_continue_entry(&pool, 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!((resume.last_episode, resume.last_watched), (7, 200));
        assert_eq!(import(&pool, &[entry(1, 7, 999)]).await.unwrap(), 0);
        let timestamp: i64 =
            sqlx::query_scalar("SELECT watched_at FROM history WHERE anime_id=1 AND episode=5")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(timestamp, 200);
    }
    #[tokio::test]
    async fn both_accounts_converge_and_do_not_loop() {
        let pool = crate::db::init(":memory:").await.unwrap();
        super::super::tables(&pool).await.unwrap();
        import(&pool, &[entry(1, 6, 100)]).await.unwrap();
        import(&pool, &[entry(1, 3, 200)]).await.unwrap();
        assert_eq!(
            catch_up(&pool, "anilist", 11, &[entry(1, 6, 100)])
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            catch_up(&pool, "mal", 22, &[entry(1, 3, 200)])
                .await
                .unwrap(),
            1
        );
        let job: (i64, i64) =
            sqlx::query_as("SELECT progress,user_id FROM tracker_jobs WHERE provider='mal'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(job, (6, 22));
        assert_eq!(
            catch_up(&pool, "mal", 22, &[entry(1, 6, 300)])
                .await
                .unwrap(),
            0
        );
    }
    #[tokio::test]
    async fn zero_and_invalid_counts_do_not_create_history() {
        let pool = crate::db::init(":memory:").await.unwrap();
        assert_eq!(import(&pool, &[entry(1, 0, 100)]).await.unwrap(), 0);
        assert!(import(&pool, &[entry(1, -1, 100)]).await.is_err());
        assert!(import(&pool, &[entry(1, 10_001, 100)]).await.is_err());
        assert!(crate::db::user::get_continue_watching(&pool)
            .await
            .unwrap()
            .is_empty());
    }
}
