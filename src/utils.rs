use crate::secrets::Secrets;
use crate::APP_IDENTIFIER;
use anyhow::anyhow;
use log::{debug, error};
use std::io::Write;
use std::path::{Path, PathBuf};
use you_have_mail_common::{EncryptionKey, ExposeSecret, Secret};

#[cfg(unix)]
pub fn create_dir_user_only(p: impl AsRef<Path>) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(p.as_ref())
}

#[cfg(not(unix))]
pub fn create_dir_user_only(p: impl AsRef<Path>) -> std::io::Result<()> {
    return std::fs::create_dir_all(p);
}

#[cfg(unix)]
pub fn write_user_file(p: impl AsRef<Path>, content: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(p.as_ref())?;
    file.write_all(content)
}

#[cfg(not(unix))]
pub fn write_user_file(p: impl AsRef<Path>, content: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(p.as_ref())?;
    file.write_all(content)
}

pub fn get_default_config_dir() -> anyhow::Result<PathBuf> {
    let config_dir =
        dirs::config_local_dir().ok_or(anyhow!("Failed to get configuration directory"))?;
    let config_dir = config_dir.join(APP_IDENTIFIER);
    Ok(config_dir)
}

pub fn get_default_log_dir() -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_dir().ok_or(anyhow!("Failed to get data directory"))?;
    let data_dir = data_dir.join(APP_IDENTIFIER);
    Ok(data_dir)
}

pub fn get_config_file_path(p: impl AsRef<Path>) -> PathBuf {
    return p.as_ref().join("config");
}

pub enum GetSecretKeyState {
    New(Secret<EncryptionKey>),
    Existing(Secret<EncryptionKey>),
}

pub fn get_or_create_secret_key(secrets: &mut dyn Secrets) -> anyhow::Result<GetSecretKeyState> {
    let key = secrets.load().map_err(|e| {
        error!("{e}");
        e
    })?;
    if let Some(key) = key {
        debug!("Found existing encryption key");
        return Ok(GetSecretKeyState::Existing(key));
    }

    debug!("No key found, generating new one");
    let new_key = EncryptionKey::new();

    debug!("Storing new encryption key");
    secrets.store(new_key.expose_secret()).map_err(|e| {
        error!("{e}");
        e
    })?;

    Ok(GetSecretKeyState::New(new_key))
}
