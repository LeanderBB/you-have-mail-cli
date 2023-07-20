use crate::secrets::Secrets;
use crate::APP_IDENTIFIER;
use anyhow::anyhow;
use you_have_mail_common::{EncryptionKey, Secret};

/// Store secrets use OS's keychain implementation.
pub struct KeyringSecrets {
    entry: keyring::Entry,
}

impl KeyringSecrets {
    pub fn new() -> anyhow::Result<Self> {
        const KEY_RING_USER: &str = "YouHaveMailCLI";
        let entry = keyring::Entry::new(APP_IDENTIFIER, KEY_RING_USER)
            .map_err(|e| anyhow!("Failed to get keyring entry:{e}"))?;

        Ok(Self { entry })
    }
}

impl Secrets for KeyringSecrets {
    fn store(&mut self, key: &EncryptionKey) -> anyhow::Result<()> {
        self.entry
            .set_password(&key.to_base64())
            .map_err(|e| anyhow!("Failed to store key: {e}"))
    }

    fn load(&mut self) -> anyhow::Result<Option<Secret<EncryptionKey>>> {
        let key_str = match self.entry.get_password() {
            Ok(s) => s,
            Err(e) => {
                return match e {
                    keyring::Error::NoEntry => Ok(None),
                    _ => Err(anyhow!("Failed to load key: {e}")),
                };
            }
        };

        let key = EncryptionKey::with_base64(key_str).map_err(|_| anyhow!("Invalid key format"))?;
        Ok(Some(Secret::new(key)))
    }
}
