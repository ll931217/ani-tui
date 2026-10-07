//! Resolve aired episodes separately from a season's planned episode count.

use crate::db::cache::Anime;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::time::Duration;

const QUERY: &str = r#"query($id: Int) {
    Media(id: $id, type: ANIME) {
        status episodes
        nextAiringEpisode { episode airingAt }
    }
    Page(page: 1, perPage: 1) {
        airingSchedules(mediaId: $id, notYetAired: false, sort: EPISODE_DESC) {
            episode airingAt
        }
    }
}"#;

fn bounded_count(value: i64) -> Option<u32> {
    u32::try_from(value).ok().filter(|count| *count <= 10_000)
}

/// Counts that can be established without consulting an airing schedule.
pub fn known_count(anime: &Anime) -> Option<u32> {
    status_count(anime.status.as_deref(), anime.episodes)
}

fn status_count(status: Option<&str>, episodes: Option<i64>) -> Option<u32> {
    match status {
        Some("FINISHED") => episodes.filter(|count| *count > 0).and_then(bounded_count),
        Some("NOT_YET_RELEASED") => Some(0),
        _ => None,
    }
}

/// Parse only evidence of episodes aired by `now`, never the planned total.
pub(crate) fn parse_count(media: &Value, now: i64) -> Option<u32> {
    if let Some(count) = status_count(media["status"].as_str(), media["episodes"].as_i64()) {
        return Some(count);
    }
    let scheduled = media["airingSchedule"]["nodes"]
        .as_array()
        .and_then(|nodes| {
            nodes
                .iter()
                .filter_map(|node| {
                    let airing_at = node["airingAt"].as_i64()?;
                    let episode = node["episode"].as_i64()?;
                    (airing_at <= now && episode > 0)
                        .then(|| bounded_count(episode))
                        .flatten()
                })
                .max()
        });
    let next = &media["nextAiringEpisode"];
    let previous = next["airingAt"]
        .as_i64()
        .filter(|time| *time > now)
        .and_then(|_| {
            next["episode"]
                .as_i64()
                .filter(|episode| *episode > 0)
                .and_then(|episode| bounded_count(episode - 1))
        });
    scheduled.into_iter().chain(previous).max()
}

/// Join the Media metadata and the sorted root Page schedule response.
fn parse_response(response: &Value, now: i64) -> Option<u32> {
    if response.get("errors").is_some() { return None; }
    let mut media = response["data"]["Media"].as_object()?.clone();
    media.insert("airingSchedule".to_owned(), json!({
        "nodes": response["data"]["Page"]["airingSchedules"]
    }));
    parse_count(&Value::Object(media), now)
}

/// Fetch an aired count, caching successful evidence for fifteen minutes.
/// On network failure, a previously confirmed count remains a safe fallback.
pub async fn aired_count(pool: &SqlitePool, anime: &Anime, now: i64) -> Option<u32> {
    if let Some(count) = known_count(anime) {
        return Some(count);
    }
    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS episode_airing (anime_id INTEGER PRIMARY KEY, aired INTEGER NOT NULL, checked_at INTEGER NOT NULL)").execute(pool).await;
    let cached = sqlx::query_as::<_, (i64, i64)>(
        "SELECT aired, checked_at FROM episode_airing WHERE anime_id = ?",
    )
    .bind(anime.id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .and_then(|(count, checked)| bounded_count(count).map(|count| (count, checked)));
    if let Some((count, checked)) = cached {
        if now >= checked && now.saturating_sub(checked) < 900 {
            return Some(count);
        }
    }
    let fresh = async {
        let client = reqwest::Client::builder()
            .user_agent("ani-tui")
            .timeout(Duration::from_secs(10))
            .build()
            .ok()?;
        let response: Value = client
            .post("https://graphql.anilist.co")
            .json(&json!({"query": QUERY, "variables": {"id": anime.id}}))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()?;
        parse_response(&response, now)
    }
    .await;
    if let Some(count) = fresh {
        let _ = sqlx::query("INSERT INTO episode_airing (anime_id, aired, checked_at) VALUES (?, ?, ?) ON CONFLICT(anime_id) DO UPDATE SET aired = excluded.aired, checked_at = excluded.checked_at")
            .bind(anime.id).bind(i64::from(count)).bind(now).execute(pool).await;
    }
    fresh.or(cached.map(|(count, _)| count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_page_response_confirms_nia_listons_first_episode() {
        let response = json!({"data": {
            "Media": {"status":"RELEASING","episodes":25,
                "nextAiringEpisode":{"episode":2,"airingAt":1791894360}},
            "Page": {"airingSchedules":[{"episode":1,"airingAt":1791289560}]}
        }});
        assert_eq!(parse_response(&response, 1791379200), Some(1));
        assert_eq!(parse_response(&json!({"errors":[{"message":"Invalid query"}],"data":null}),1791379200),None);
    }

    #[test]
    fn planned_total_does_not_become_aired_count() {
        let media = json!({"status":"RELEASING", "episodes":12,
            "nextAiringEpisode":{"episode":3,"airingAt":200}});
        assert_eq!(parse_count(&media, 100), Some(2));
    }

    #[test]
    fn first_future_episode_means_none_aired() {
        assert_eq!(
            parse_count(
                &json!({"nextAiringEpisode":{"episode":1,"airingAt":200}}),
                100
            ),
            Some(0)
        );
    }

    #[test]
    fn completed_and_unreleased_statuses_are_known() {
        assert_eq!(
            parse_count(&json!({"status":"FINISHED","episodes":12}), 100),
            Some(12)
        );
        assert_eq!(
            parse_count(&json!({"status":"NOT_YET_RELEASED","episodes":12}), 100),
            Some(0)
        );
        assert_eq!(parse_count(&json!({"status":"CANCELLED"}), 100), None);
        assert_eq!(
            parse_count(
                &json!({"status":"CANCELLED","airingSchedule":{"nodes":[{"episode":2,"airingAt":90}]}}),
                100
            ),
            Some(2)
        );
    }

    #[test]
    fn missing_negative_and_excessive_counts_remain_unknown() {
        for episodes in [Value::Null, json!(-1), json!(0), json!(10001)] {
            assert_eq!(
                parse_count(&json!({"status":"FINISHED","episodes":episodes}), 100),
                None
            );
        }
        assert_eq!(
            parse_count(&json!({"status":"RELEASING","episodes":12}), 100),
            None
        );
    }

    #[test]
    fn only_past_schedule_nodes_count() {
        let media = json!({"airingSchedule":{"nodes":[
            {"episode":4,"airingAt":101}, {"episode":2,"airingAt":100},
            {"episode":-1,"airingAt":90}, {"episode":10001,"airingAt":90}]}});
        assert_eq!(parse_count(&media, 100), Some(2));
        assert_eq!(
            parse_count(
                &json!({"airingSchedule":{"nodes":[{"episode":3,"airingAt":200}]}}),
                100
            ),
            None
        );
    }

    #[test]
    fn expired_next_episode_is_not_assumed_aired() {
        assert_eq!(
            parse_count(
                &json!({"nextAiringEpisode":{"episode":3,"airingAt":99}}),
                100
            ),
            None
        );
        assert_eq!(
            parse_count(
                &json!({"nextAiringEpisode":{"episode":-1,"airingAt":200}}),
                100
            ),
            None
        );
    }
    #[tokio::test]
    async fn fresh_cache_uses_aired_count_instead_of_planned_total() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE episode_airing (anime_id INTEGER PRIMARY KEY, aired INTEGER NOT NULL, checked_at INTEGER NOT NULL)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO episode_airing VALUES (1, 2, 100)")
            .execute(&pool)
            .await
            .unwrap();
        let mut anime: Anime = serde_json::from_value(json!({
            "id":1, "title_english":null, "title_romaji":"Example",
            "title_native":null, "description":null, "episodes":12,
            "status":"RELEASING", "season":null, "season_year":null,
            "score":null, "format":null, "genres":"[]", "cover_url":null,
            "cover_blob":null, "has_dub":0, "updated_at":100
        }))
        .unwrap();
        assert_eq!(aired_count(&pool, &anime, 999).await, Some(2));
        anime.id = 2;
        anime.status = Some("NOT_YET_RELEASED".into());
        assert_eq!(aired_count(&pool, &anime, 999).await, Some(0));
        anime.status = Some("FINISHED".into());
        assert_eq!(aired_count(&pool, &anime, 999).await, Some(12));
    }
}
