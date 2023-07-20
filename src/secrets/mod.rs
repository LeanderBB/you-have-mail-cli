//! Collection of secret storage services.
use serde::Deserialize;
use std::path::Path;
use you_have_mail_common::{EncryptionKey, Secret};

mod plain_secrets;

#[cfg(feature = "keyring-secrets")]
mod keyring_secrets;

/// Controls how the secret encryption is stored on disk.
pub trait Secrets {
    /// Store the encryption key in the secret store.
    fn store(&mut self, key: &EncryptionKey) -> anyhow::Result<()>;
    /// Load encryption key from secret store.
    fn load(&mut self) -> anyhow::Result<Option<Secret<EncryptionKey>>>;
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Deserialize)]
pub enum SecretsType {
    Plain,
    Keyring,
}

pub fn new_secrets(t: SecretsType, config_dir: &Path) -> Result<Box<dyn Secrets>, anyhow::Error> {
    match t {
        SecretsType::Plain => Ok(Box::new(plain_secrets::PlainSecrets::with_directory(
            config_dir,
        )?)),
        #[cfg(feature = "keyring-secrets")]
        SecretsType::Keyring => Ok(Box::new(keyring_secrets::KeyringSecrets::new()?)),
        #[cfg(not(feature = "keyring-secrets"))]
        SecretsType::Keyring => {
            use anyhow::anyhow;
            Err(anyhow!("keyring-secrets feature is not enabled"))
        }
    }
}
