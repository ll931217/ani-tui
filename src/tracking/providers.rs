use super::credentials::Account;
use crate::db::cache::Anime;
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;

pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("ani-tui/1.0.6")
        .build()?)
}
pub async fn graphql(
    client: &Client,
    token: Option<&str>,
    query: &str,
    variables: Value,
) -> Result<Value> {
    let mut request = client
        .post("https://graphql.anilist.co")
        .json(&json!({"query": query, "variables":variables}));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("AniList connection failed"))?;
    if !response.status().is_success() {
        bail!("AniList request failed ({})", response.status().as_u16());
    }
    let value: Value = response.json().await.context("Invalid AniList response")?;
    if value.get("errors").is_some() {
        bail!("AniList rejected the request; check account authorization");
    }
    Ok(value)
}
pub async fn identity(client: &Client, provider: &str, token: &str) -> Result<(String, i64)> {
    let value = if provider == "anilist" {
        graphql(
            client,
            Some(token),
            "query { Viewer { id name } }",
            json!({}),
        )
        .await?["data"]["Viewer"]
            .clone()
    } else {
        let response = client
            .get("https://api.myanimelist.net/v2/users/@me")
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("MyAnimeList connection failed"))?;
        if !response.status().is_success() {
            bail!("MyAnimeList authorization rejected");
        }
        response
            .json::<Value>()
            .await
            .context("Invalid MyAnimeList response")?
    };
    let name = value["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .context("Account name missing")?;
    let id = value["id"]
        .as_i64()
        .filter(|id| *id > 0)
        .context("Account ID missing")?;
    Ok((name.chars().filter(|ch| !ch.is_control()).collect(), id))
}
pub async fn refresh(client: &Client, account: &mut Account, now: i64) -> Result<()> {
    if account.expires_at.is_none_or(|expires| expires > now + 60) {
        return Ok(());
    }
    let refresh = account
        .refresh_token
        .as_deref()
        .context("Reconnect MyAnimeList to renew authorization")?;
    let response = client
        .post("https://myanimelist.net/v1/oauth2/token")
        .form(&[
            ("client_id", account.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
        ])
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("MyAnimeList renewal failed"))?;
    if !response.status().is_success() {
        bail!("MyAnimeList authorization expired; reconnect account");
    }
    let data: Value = response
        .json()
        .await
        .context("Invalid authorization response")?;
    account.token = data["access_token"]
        .as_str()
        .context("Authorization token missing")?
        .into();
    if let Some(token) = data["refresh_token"].as_str() {
        account.refresh_token = Some(token.into());
    }
    account.expires_at = Some(now + data["expires_in"].as_i64().unwrap_or(3600));
    Ok(())
}
fn completed(anime: &Anime, progress: i64) -> bool {
    anime.status.as_deref() == Some("FINISHED")
        && anime.episodes.is_some_and(|n| n > 0 && progress >= n)
}
fn remote_progress(entry: &Value, key: &str) -> Result<i64> {
    if entry.is_null() {
        return Ok(0);
    }
    entry[key]
        .as_i64()
        .filter(|progress| *progress >= 0)
        .context("Provider progress missing or invalid")
}
pub async fn sync(
    client: &Client,
    pool: &SqlitePool,
    provider: &str,
    account: &Account,
    anime: &Anime,
    progress: i64,
) -> Result<()> {
    if provider == "anilist" {
        let current = graphql(
            client,
            Some(&account.token),
            "query($id:Int!){Media(id:$id){mediaListEntry{progress status}}}",
            json!({"id":anime.id}),
        )
        .await?;
        if !current["data"]["Media"].is_object() {
            bail!("AniList anime lookup missing");
        }
        if remote_progress(&current["data"]["Media"]["mediaListEntry"], "progress")? >= progress {
            return Ok(());
        }
        graphql(client, Some(&account.token), "mutation($id:Int!,$progress:Int!,$status:MediaListStatus!){SaveMediaListEntry(mediaId:$id,progress:$progress,status:$status){id}}", json!({"id":anime.id,"progress":progress,"status":if completed(anime,progress) || current["data"]["Media"]["mediaListEntry"]["status"].as_str()==Some("COMPLETED"){"COMPLETED"}else{"CURRENT"}})).await?;
        return Ok(());
    }
    let cached: Option<i64> =
        sqlx::query_scalar("SELECT mal_id FROM tracker_ids WHERE anime_id = ?")
            .bind(anime.id)
            .fetch_optional(pool)
            .await?;
    let mal_id = match cached {
        Some(id) => id,
        None => {
            let data = graphql(
                client,
                None,
                "query($id:Int!){Media(id:$id){idMal}}",
                json!({"id":anime.id}),
            )
            .await?;
            let id = data["data"]["Media"]["idMal"]
                .as_i64()
                .filter(|id| *id > 0)
                .context("No authoritative MyAnimeList mapping for this anime")?;
            sqlx::query("INSERT OR REPLACE INTO tracker_ids(anime_id,mal_id) VALUES(?,?)")
                .bind(anime.id)
                .bind(id)
                .execute(pool)
                .await?;
            id
        }
    };
    let url = format!("https://api.myanimelist.net/v2/anime/{mal_id}");
    let response = client
        .get(&url)
        .query(&[("fields", "my_list_status")])
        .bearer_auth(&account.token)
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("MyAnimeList connection failed"))?;
    if !response.status().is_success() {
        bail!("MyAnimeList lookup failed ({})", response.status().as_u16());
    }
    let current: Value = response
        .json()
        .await
        .context("Invalid MyAnimeList response")?;
    if current["id"].as_i64() != Some(mal_id) {
        bail!("MyAnimeList anime lookup mismatch");
    }
    if remote_progress(&current["my_list_status"], "num_episodes_watched")? >= progress {
        return Ok(());
    }
    let response = client
        .patch(format!("{url}/my_list_status"))
        .bearer_auth(&account.token)
        .form(&[
            (
                "status",
                if completed(anime, progress)
                    || current["my_list_status"]["status"].as_str() == Some("completed")
                {
                    "completed"
                } else {
                    "watching"
                }
                .to_string(),
            ),
            ("num_watched_episodes", progress.to_string()),
        ])
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("MyAnimeList progress update failed"))?;
    if !response.status().is_success() {
        bail!("MyAnimeList update failed ({})", response.status().as_u16());
    }
    let updated: Value = response.json().await.context("Invalid MyAnimeList update response")?;
    if remote_progress(&updated, "num_episodes_watched")? < progress {
        bail!("MyAnimeList did not confirm the requested progress");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_progress_rejects_malformed_values() {
        assert_eq!(remote_progress(&Value::Null, "progress").unwrap(), 0);
        assert_eq!(
            remote_progress(&json!({"progress":9}), "progress").unwrap(),
            9
        );
        assert!(remote_progress(&json!({}), "progress").is_err());
        assert!(remote_progress(&json!({"progress":-1}), "progress").is_err());
        assert!(remote_progress(&json!({"progress":"9"}), "progress").is_err());
    }
}
