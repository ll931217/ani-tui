//! Read account progress using provider IDs, never title matching.
use super::{credentials::Account, providers::graphql};
use crate::db::cache::{self, Anime};
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

const MAX_ENTRIES: usize = 20_000;
const FIELDS: &str = "id idMal title{english romaji native} description(asHtml:false) episodes status season seasonYear averageScore format genres coverImage{large}";

pub(super) struct RemoteEntry {
    pub anime: Anime,
    pub progress: i64,
    pub updated_at: i64,
}
pub(super) struct RemoteList {
    pub entries: Vec<RemoteEntry>,
    pub unmapped: usize,
}

pub(super) async fn fetch(
    client: &Client,
    pool: &SqlitePool,
    provider: &str,
    account: &Account,
    now: i64,
) -> Result<RemoteList> {
    match provider {
        "anilist" => anilist(client, pool, account, now).await,
        "mal" => mal(client, pool, account, now).await,
        _ => bail!("Unknown progress provider"),
    }
}

fn bounded(value: &Value, name: &str, maximum: i64) -> Result<i64> {
    value
        .as_i64()
        .filter(|n| (0..=maximum).contains(n))
        .with_context(|| format!("Invalid remote {name}"))
}
fn anime(media: &Value, now: i64) -> Result<Anime> {
    let id = bounded(&media["id"], "anime ID", i32::MAX as i64)?;
    if id == 0 {
        bail!("Invalid remote anime ID");
    }
    let title = media["title"]["romaji"]
        .as_str()
        .filter(|s| !s.is_empty())
        .context("Remote anime title missing")?;
    let genres = media["genres"]
        .as_array()
        .context("Remote genres missing")?;
    if genres.iter().any(|v| !v.is_string()) {
        bail!("Invalid remote genres");
    }
    Ok(Anime {
        id,
        title_romaji: title.into(),
        title_english: media["title"]["english"].as_str().map(str::to_owned),
        title_native: media["title"]["native"].as_str().map(str::to_owned),
        description: media["description"].as_str().map(str::to_owned),
        episodes: media["episodes"].as_i64(),
        status: media["status"].as_str().map(str::to_owned),
        season: media["season"].as_str().map(str::to_owned),
        season_year: media["seasonYear"].as_i64(),
        score: media["averageScore"].as_i64(),
        format: media["format"].as_str().map(str::to_owned),
        genres: serde_json::to_string(genres)?,
        cover_url: media["coverImage"]["large"].as_str().map(str::to_owned),
        cover_blob: None,
        has_dub: 0,
        updated_at: now,
    })
}

fn anilist_page(data: &Value, now: i64) -> Result<(Vec<RemoteEntry>, bool)> {
    let page = &data["data"]["Page"];
    let more = page["pageInfo"]["hasNextPage"]
        .as_bool()
        .context("AniList pagination missing")?;
    let list = page["mediaList"]
        .as_array()
        .context("AniList list missing")?;
    if list.len() > 50 || (more && list.len() != 50) {
        bail!("Invalid AniList pagination");
    }
    let entries = list
        .iter()
        .map(|entry| {
            Ok(RemoteEntry {
                anime: anime(&entry["media"], now)?,
                progress: bounded(&entry["progress"], "progress", 10_000)?,
                updated_at: bounded(&entry["updatedAt"], "update time", now.saturating_add(300))?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((entries, more))
}

async fn anilist(
    client: &Client,
    pool: &SqlitePool,
    account: &Account,
    now: i64,
) -> Result<RemoteList> {
    let query = ["query($user:Int!,$page:Int!){Page(page:$page,perPage:50){pageInfo{hasNextPage} mediaList(userId:$user,type:ANIME,sort:MEDIA_ID){progress updatedAt media{", FIELDS, "}}}}"].concat();
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for page in 1..=MAX_ENTRIES / 50 {
        let response = graphql(
            client,
            Some(&account.token),
            &query,
            json!({"user":account.user_id,"page":page}),
        )
        .await?;
        let (batch, more) = anilist_page(&response, now)?;
        for entry in batch {
            if !seen.insert(entry.anime.id) {
                bail!("AniList list changed during pagination; retry sync");
            }
            if let Some(id) = response["data"]["Page"]["mediaList"]
                .as_array()
                .and_then(|items| {
                    items
                        .iter()
                        .find(|v| v["media"]["id"].as_i64() == Some(entry.anime.id))
                })
                .and_then(|v| v["media"]["idMal"].as_i64())
                .filter(|id| *id > 0)
            {
                sqlx::query("INSERT OR REPLACE INTO tracker_ids(anime_id,mal_id) VALUES(?,?)")
                    .bind(entry.anime.id)
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
            entries.push(entry);
        }
        if !more {
            return Ok(RemoteList {
                entries,
                unmapped: 0,
            });
        }
    }
    bail!("AniList list exceeds the safe import limit")
}

type MalRows = Vec<(i64, i64, String)>;

fn mal_page(data: &Value) -> Result<(MalRows, bool)> {
    let list = data["data"]
        .as_array()
        .context("MyAnimeList list missing")?;
    let paging = data["paging"]
        .as_object()
        .context("MyAnimeList pagination missing")?;
    let more = match paging.get("next") {
        None | Some(Value::Null) => false,
        Some(Value::String(next)) if !next.is_empty() => true,
        _ => bail!("Invalid MyAnimeList pagination"),
    };
    if list.len() > 1000 || (more && list.len() != 1000) {
        bail!("Invalid MyAnimeList page");
    }
    let rows = list
        .iter()
        .map(|entry| {
            let id = bounded(&entry["node"]["id"], "MyAnimeList ID", i32::MAX as i64)?;
            if id == 0 {
                bail!("Invalid MyAnimeList ID");
            }
            let status = &entry["list_status"];
            let progress = bounded(&status["num_episodes_watched"], "progress", 10_000)?;
            let date = status["updated_at"]
                .as_str()
                .context("MyAnimeList update time missing")?;
            if date.len() > 40 || !date.contains('T') {
                bail!("Invalid MyAnimeList update time");
            }
            Ok((id, progress, date.to_owned()))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((rows, more))
}

async fn timestamp(pool: &SqlitePool, date: &str, now: i64) -> Result<i64> {
    let value: Option<i64> = sqlx::query_scalar("SELECT CAST(strftime('%s', ?) AS INTEGER)")
        .bind(date)
        .fetch_one(pool)
        .await?;
    value
        .filter(|v| (0..=now.saturating_add(300)).contains(v))
        .context("Invalid MyAnimeList update time")
}

async fn mapped_anime(
    client: &Client,
    pool: &SqlitePool,
    ids: &[i64],
    now: i64,
) -> Result<HashMap<i64, Anime>> {
    let mut mapped = HashMap::new();
    let mut missing = Vec::new();
    for &id in ids {
        let cached: Option<i64> =
            sqlx::query_scalar("SELECT anime_id FROM tracker_ids WHERE mal_id=?")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        if let Some(anime_id) = cached {
            if let Some(anime) = cache::get_anime(pool, anime_id).await? {
                mapped.insert(id, anime);
                continue;
            }
        }
        missing.push(id);
    }
    for batch in missing.chunks(20) {
        // A Page query returns an empty array for absent mappings, unlike Media's 404.
        let query = ["query($ids:[Int]){Page(page:1,perPage:50){pageInfo{hasNextPage} media(idMal_in:$ids,type:ANIME){", FIELDS, "}}}"].concat();
        let response = graphql(client, None, &query, json!({"ids":batch})).await?;
        let resolved = mapping_page(&response, batch, now)?;
        for (id, mut anime) in resolved {
            if let Some(existing) = cache::get_anime(pool, anime.id).await? {
                anime.has_dub = existing.has_dub;
            }
            // Warm each batch durably so a later provider failure doesn't repeat lookups.
            cache::upsert_anime(pool, &anime).await?;
            sqlx::query("INSERT OR REPLACE INTO tracker_ids(anime_id,mal_id) VALUES(?,?)")
                .bind(anime.id)
                .bind(id)
                .execute(pool)
                .await?;
            mapped.insert(id, anime);
        }
    }
    Ok(mapped)
}

fn mapping_page(response: &Value, requested: &[i64], now: i64) -> Result<HashMap<i64, Anime>> {
    let page = &response["data"]["Page"];
    if page["pageInfo"]["hasNextPage"].as_bool() != Some(false) {
        bail!("Incomplete AniList mapping response");
    }
    let media = page["media"]
        .as_array()
        .context("AniList mapping response missing")?;
    let mut mapped = HashMap::new();
    for value in media {
        let id = value["idMal"]
            .as_i64()
            .context("AniList mapping ID missing")?;
        if !requested.contains(&id) || mapped.contains_key(&id) {
            bail!("AniList MyAnimeList mapping mismatch");
        }
        mapped.insert(id, anime(value, now)?);
    }
    Ok(mapped)
}

async fn mal(
    client: &Client,
    pool: &SqlitePool,
    account: &Account,
    now: i64,
) -> Result<RemoteList> {
    let mut result = RemoteList {
        entries: Vec::new(),
        unmapped: 0,
    };
    let mut seen = HashSet::new();
    for offset in (0..MAX_ENTRIES).step_by(1000) {
        // Ignore provider-supplied next URLs: tokens only go to the fixed provider endpoint.
        let response = client
            .get("https://api.myanimelist.net/v2/users/@me/animelist")
            .query(&[
                ("fields", "list_status".to_owned()),
                ("limit", "1000".to_owned()),
                ("offset", offset.to_string()),
                ("sort", "anime_title".to_owned()),
                ("nsfw", "true".to_owned()),
            ])
            .bearer_auth(&account.token)
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("MyAnimeList list connection failed"))?;
        if !response.status().is_success() {
            bail!(
                "MyAnimeList list request failed ({})",
                response.status().as_u16()
            );
        }
        let data: Value = response
            .json()
            .await
            .context("Invalid MyAnimeList list response")?;
        let (rows, more) = mal_page(&data)?;
        let ids: Vec<_> = rows.iter().map(|row| row.0).collect();
        let mut mapped = mapped_anime(client, pool, &ids, now).await?;
        for (id, progress, date) in rows {
            let updated_at = timestamp(pool, &date, now).await?;
            if !seen.insert(id) {
                bail!("MyAnimeList list changed during pagination; retry sync");
            }
            match mapped.remove(&id) {
                Some(anime) => result.entries.push(RemoteEntry {
                    anime,
                    progress,
                    updated_at,
                }),
                None => result.unmapped += 1,
            }
        }
        if !more {
            return Ok(result);
        }
    }
    bail!("MyAnimeList list exceeds the safe import limit")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn media() -> Value {
        json!({"id":1,"title":{"romaji":"Anime"},"genres":[],"idMal":2})
    }
    #[test]
    fn exact_mapping_allows_missing_titles_and_rejects_mismatches() {
        let mut response =
            json!({"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[media()]}}});
        let mapped = mapping_page(&response, &[2, 3], 20).unwrap();
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[&2].id, 1);
        assert!(mapping_page(&response, &[3], 20).is_err());
        response["data"]["Page"]["pageInfo"]["hasNextPage"] = json!(true);
        assert!(mapping_page(&response, &[2], 20).is_err());
        let empty = json!({"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[]}}});
        assert!(mapping_page(&empty, &[3], 20).unwrap().is_empty());
    }
    #[test]
    fn anilist_requires_progress_and_complete_pagination() {
        let mut response = json!({"data":{"Page":{"pageInfo":{"hasNextPage":false},"mediaList":[{"media":media(),"progress":3,"updatedAt":10}]}}});
        assert_eq!(anilist_page(&response, 20).unwrap().0[0].progress, 3);
        response["data"]["Page"]["mediaList"][0]["progress"] = json!(-1);
        assert!(anilist_page(&response, 20).is_err());
        assert!(anilist_page(&json!({}), 20).is_err());
        assert!(anilist_page(
            &json!({"data":{"Page":{"pageInfo":{"hasNextPage":true},"mediaList":[]}}}),
            20
        )
        .is_err());
    }
    #[tokio::test]
    async fn mal_validates_timestamp_and_ignores_untrusted_next_url() {
        let mut response = json!({"data":[{"node":{"id":2},"list_status":{"num_episodes_watched":0,"updated_at":"2024-01-01T01:00:00+01:00"}}],"paging":{"next":"https://untrusted.test"}});
        let row = response["data"][0].clone();
        response["data"] = Value::Array(vec![row; 1000]);
        let (rows, more) = mal_page(&response).unwrap();
        assert!(more);
        assert_eq!(rows[0].0, 2);
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        assert_eq!(
            timestamp(&pool, &rows[0].2, 1_800_000_000).await.unwrap(),
            1_704_067_200
        );
        assert!(timestamp(&pool, "2024-13-01T00:00:00Z", 1_800_000_000)
            .await
            .is_err());
        response["data"][0]["list_status"]["updated_at"] = json!("invalid");
        assert!(mal_page(&response).is_err());
        assert!(mal_page(&json!({"data":[]})).is_err());
    }
}
