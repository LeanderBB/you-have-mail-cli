# You Have Mail CLI

CLI counter part for the [You Have Mail Android](https://github.com/LeanderBB/you-have-mail) application.

This version of the application is meant to be run as a service _somewhere_ and present the user with notifications
through one or various notifiers.


## Installation

```bash
cargo install --git="https://github.com/LeanderBB/you-have-mail-cli"
```
## Configuration

### Secrets Storage

* **Plain**: Store the encryption key on disk unencrypted.
* **Keyring**: Store encryption key in OS's keychain ([crate](https://crates.io/crates/keyring)).
  * Enabled by default with feature `secrets-keyring`

### Observer
The observer requires that a configuration file be present with the following entries:

```toml
# Secret storage type, use name from the `Secret Storage` section fo this README
secrets="Plain"
# Poll interval of the observer in seconds
poll_interval=15
# If using Plain secret storage, this must be set to true so you consent to the risks
accept_plain_secrets_insecure=true
# Set to true if you wish to write notifications to stdout
stdout_notifier="false"

# For each account create on entry such as the one below:
[[account]]
email = "foo@proton.me"
backend ="Proton Mail"
```

The observer will look for a config file in the OS's default config directory.
You can also specify a config directory using the `-c` or `--config-dir` arguments.

Finally you can generate a config file if none is present with the `--create-config` option.

```bash
you-have-mail-cli --create-config
```

### Notifiers 

#### StdOut
Prints notifications to stdout. Can be enabled  by setting `stdout_notifier="true"` in the config file.

#### ntfy

Send notifications to a [ntfy](https://github.com/binwiederhier/ntfy) instance. Enabled by default with feature 
`notifier-ntfy`, for each server instance add the following entry into the config
file:

```toml
[[ntfy]]
# Name of the server to identify in the logs.
name = "My Sever"
# Url of the server with topic
url = "https://..."
# Optional access token if server needs autentication. 
auth_token = "..."
```

_Note:_ Feature tested against public ntfy instances.

### Account Setup

Due to user input, accounts specified in the config file need to be setup with the `--configure-accounts` argument.
An interactive prompt will guide you through the process of configuring your account. Example:
```bash
you-have-mail-cli --configure-accounts

| INFO  | Starting You Have Mail CLI
| INFO  | Loading config from path "..."
| INFO  | Checking Config Accounts
Please type password for account:
Please Input TOTP 2FA for account: <2FACODE>
```

To remove accounts, remove them from the configuration and run with the `--delete-accounts` argument.
```bash
you-have-mail-cli --delete-accounts
```

## Supported Backends

See [You Have Mail Common](https://github.com/LeanderBB/you-have-mail-common#supported-backends) for list of supported
backends.
