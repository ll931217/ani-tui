use super::{credentials::Account, providers};
use anyhow::{bail, Context, Result};
use rand::{rngs::OsRng, RngCore};
use serde_json::Value;
use std::io::Write;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn random_secret() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn open(url: &str) {
    println!("Open this authorization URL in your browser:\n{url}");
    let _ = std::process::Command::new("xdg-open")
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
fn hidden_token() -> Result<String> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind};
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
    print!("Paste the AniList access token (input hidden), then press Enter: ");
    std::io::stdout().flush()?;
    crossterm::terminal::enable_raw_mode()
        .context("Token input requires an interactive terminal")?;
    let _restore = Restore;
    let mut token = String::new();
    loop {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            match key.code {
                KeyCode::Enter => break,
                KeyCode::Esc => bail!("Connection cancelled"),
                KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    bail!("Connection cancelled")
                }
                KeyCode::Backspace => {
                    token.pop();
                }
                KeyCode::Char(ch) if !ch.is_control() && token.len() < 8192 => token.push(ch),
                _ => {}
            }
        }
    }
    println!("\r");
    if token.trim().is_empty() {
        bail!("No token provided");
    }
    Ok(token.trim().to_string())
}
fn callback_code(target: &str, expected_state: &str) -> Result<String> {
    let url = reqwest::Url::parse(&format!("http://127.0.0.1:8766{target}"))
        .context("Invalid callback")?;
    if url.path() != "/callback" {
        bail!("Unexpected callback path");
    }
    let mut params = std::collections::HashMap::new();
    for (key, value) in url.query_pairs() {
        if params.insert(key, value).is_some() {
            bail!("Duplicate callback parameter");
        }
    }
    if params.get("state").map(|s| s.as_ref()) != Some(expected_state) {
        bail!("Authorization state mismatch");
    }
    if params.contains_key("error") {
        bail!("Authorization denied");
    }
    params
        .get("code")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .context("Authorization code missing")
}
pub async fn connect(provider: &str, client_id: &str, now: i64) -> Result<Account> {
    if client_id.is_empty() || client_id.len() > 256 {
        bail!("Invalid client ID");
    }
    let client = providers::client()?;
    let (token, refresh_token, expires_at) = if provider == "anilist" {
        let mut url = reqwest::Url::parse("https://anilist.co/api/v2/oauth/authorize")?;
        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("response_type", "token");
        open(url.as_str());
        let token = tokio::task::spawn_blocking(hidden_token).await??;
        (token, None, None)
    } else {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:8766")
            .await
            .context("Cannot bind OAuth callback port 8766")?;
        let state = random_secret();
        let verifier = random_secret();
        let redirect = "http://127.0.0.1:8766/callback";
        let mut url = reqwest::Url::parse("https://myanimelist.net/v1/oauth2/authorize")?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect)
            .append_pair("state", &state)
            .append_pair("code_challenge", &verifier)
            .append_pair("code_challenge_method", "plain");
        open(url.as_str());
        let code = tokio::time::timeout(std::time::Duration::from_secs(300), async {
            loop {
                let (mut stream, _) = listener.accept().await?;
                let mut buffer = vec![0u8;8192];
                let length = match tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    let mut length=0;
                    loop {
                        if length == buffer.len() { return Err(std::io::Error::other("Callback headers too large")); }
                        let count=stream.read(&mut buffer[length..]).await?;
                        if count==0 { return Err(std::io::Error::other("Incomplete callback")); }
                        length+=count;
                        if buffer[..length].windows(4).any(|part| part==b"\r\n\r\n") { return Ok(length); }
                    }
                }).await { Ok(Ok(n)) => n, _ => continue };
                let first = std::str::from_utf8(&buffer[..length]).unwrap_or("").lines().next().unwrap_or("");
                let target = first.strip_prefix("GET ").and_then(|line| line.split_whitespace().next()).unwrap_or("");
                if target.starts_with("/favicon") { continue; }
                let result = callback_code(target, &state);
                let message = if result.is_ok() { "Account authorization received. Return to ani-tui." } else { "Authorization callback rejected. Return to ani-tui." };
                let response = format!("HTTP/1.1 {}\r\nContent-Type: text/plain\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",if result.is_ok(){"200 OK"}else{"400 Bad Request"},message.len(),message);
                let _ = stream.write_all(response.as_bytes()).await;
                if result.is_ok() || result.as_ref().is_err_and(|error| error.to_string() == "Authorization denied") { return result; }
            }
        }).await.context("Authorization timed out after five minutes")??;
        let response = client
            .post("https://myanimelist.net/v1/oauth2/token")
            .form(&[
                ("client_id", client_id),
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("code_verifier", verifier.as_str()),
                ("redirect_uri", redirect),
            ])
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Authorization exchange failed"))?;
        if !response.status().is_success() {
            bail!("MyAnimeList token exchange rejected; check app registration");
        }
        let data: Value = response.json().await.context("Invalid token response")?;
        (
            data["access_token"]
                .as_str()
                .context("Token missing")?
                .to_string(),
            data["refresh_token"].as_str().map(str::to_owned),
            Some(now + data["expires_in"].as_i64().unwrap_or(3600)),
        )
    };
    let (username, user_id) = providers::identity(&client, provider, &token).await?;
    Ok(Account {
        token,
        refresh_token,
        expires_at,
        client_id: client_id.into(),
        username,
        user_id,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_validates_state_and_denial() {
        assert_eq!(
            callback_code("/callback?state=abc&code=hello", "abc").unwrap(),
            "hello"
        );
        assert!(callback_code("/callback?state=other&code=hello", "abc").is_err());
        assert!(callback_code("/callback?state=abc&error=access_denied", "abc").is_err());
        assert!(callback_code("/callback?state=abc", "abc").is_err());
        assert!(callback_code("/callback?state=abc&state=abc&code=x", "abc").is_err());
    }
    #[test]
    fn pkce_entropy_and_length() {
        let a = random_secret();
        assert_eq!(a.len(), 64);
        assert_ne!(a, random_secret());
    }
}
