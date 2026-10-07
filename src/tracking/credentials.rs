use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::Path,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub client_id: String,
    pub username: String,
    pub user_id: i64,
}
pub type Accounts = BTreeMap<String, Account>;

pub async fn lock() -> Result<fs::File> {
    tokio::task::spawn_blocking(|| -> Result<fs::File> {
        let path = crate::config::Config::config_dir()?.join("accounts.lock");
        fs::create_dir_all(path.parent().context("Invalid lock path")?)?;
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(0x20000);
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(0x100);
        }
        let file = options.open(path)?;
        if !file.metadata()?.is_file() {
            bail!("Invalid account lock file");
        }
        file.lock()?;
        Ok(file)
    })
    .await?
}

pub fn load() -> Result<Accounts> {
    read(&crate::config::Config::config_dir()?.join("accounts.json"))
}
pub fn save(accounts: &Accounts) -> Result<()> {
    write(
        &crate::config::Config::config_dir()?.join("accounts.json"),
        accounts,
    )
}
fn read(path: &Path) -> Result<Accounts> {
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        bail!("Account credentials must be a regular file");
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x20000); // Linux O_NOFOLLOW: reject final symlink atomically.
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x100);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Accounts::new()),
        Err(_) => bail!("Cannot read account credentials (regular file required)"),
    };
    let meta = file.metadata()?;
    if !meta.is_file() {
        bail!("Account credentials must be a regular file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if meta.nlink() != 1 {
            bail!("Account credentials must not have hard links");
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    if meta.len() > 1_048_576 {
        bail!("Account credentials file too large");
    }
    let mut bytes = Vec::new();
    file.take(1_048_577).read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        bail!("Account credentials file too large");
    }
    serde_json::from_slice(&bytes).context("Invalid account credentials file")
}
fn write(path: &Path, accounts: &Accounts) -> Result<()> {
    let parent = path.parent().context("Invalid credentials path")?;
    fs::create_dir_all(parent)?;
    if fs::symlink_metadata(path).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        bail!("Account credentials must be a regular file");
    }
    let temp = parent.join(format!(
        ".accounts-{}-{}.tmp",
        std::process::id(),
        rand::random::<u64>()
    ));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> Result<()> {
        let mut file = options.open(&temp)?;
        file.write_all(&serde_json::to_vec(accounts)?)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.context("Cannot save account credentials")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_roundtrip_and_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        let mut accounts = Accounts::new();
        accounts.insert(
            "anilist".into(),
            Account {
                token: "secret".into(),
                refresh_token: None,
                expires_at: None,
                client_id: "123".into(),
                username: "test".into(),
                user_id: 1,
            },
        );
        write(&path, &accounts).unwrap();
        assert_eq!(read(&path).unwrap()["anilist"].token, "secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        fs::write(&target, "{}").unwrap();
        let path = dir.path().join("accounts.json");
        std::os::unix::fs::symlink(target, &path).unwrap();
        assert!(read(&path).is_err());
        assert!(write(&path, &Accounts::new()).is_err());
    }
}
