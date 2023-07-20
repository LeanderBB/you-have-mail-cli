use crate::secrets::Secrets;
use crate::utils::{create_dir_user_only, write_user_file};
use anyhow::anyhow;
use std::path::{Path, PathBuf};
use you_have_mail_common::{EncryptionKey, Secret};

/// Store secrets *UNENCRYPTED* on disk in a file.
pub struct PlainSecrets {
    filepath: PathBuf,
}

impl PlainSecrets {
    const FILENAME: &'static str = "encryption_key";

    /// Use a custom directory to store the secret key
    pub fn with_directory(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        create_dir_user_only(path.as_ref())
            .map_err(|e| anyhow!("Failed to creat config dir:{e}"))?;
        Ok(Self {
            filepath: path.as_ref().join(Self::FILENAME),
        })
    }
}

impl Secrets for PlainSecrets {
    fn store(&mut self, key: &EncryptionKey) -> anyhow::Result<()> {
        write_user_file(&self.filepath, key.as_ref())
            .map_err(|e| anyhow!("Failed to write key to disk: {e}"))
    }

    fn load(&mut self) -> anyhow::Result<Option<Secret<EncryptionKey>>> {
        let contents = match std::fs::read(&self.filepath) {
            Ok(c) => c,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(None);
                }
                return Err(anyhow!("Failed to open file :{e}"));
            }
        };

        let key = EncryptionKey::try_from(contents.as_slice())
            .map_err(|_| anyhow!("Invalid encryption key"))?;
        Ok(Some(Secret::new(key)))
    }
}
