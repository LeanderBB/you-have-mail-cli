#![cfg_attr(docsrs, feature(doc_cfg))]
// Enable clippy if our Cargo.toml file asked us to do so.
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]
// Enable as many useful Rust and Clippy warnings as we can stand.
#![warn(
    missing_copy_implementations,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    trivial_casts,
    unused_qualifications
)]
#![cfg_attr(feature = "clippy", warn(cast_possible_truncation))]
#![cfg_attr(feature = "clippy", warn(cast_possible_wrap))]
#![cfg_attr(feature = "clippy", warn(cast_precision_loss))]
#![cfg_attr(feature = "clippy", warn(cast_sign_loss))]
#![cfg_attr(feature = "clippy", warn(missing_docs_in_private_items))]
#![cfg_attr(feature = "clippy", warn(mut_mut))]
// Disallow `println!`. Use `debug!` for debug output
// (which is provided by the `log` crate).
#![cfg_attr(feature = "clippy", warn(print_stdout))]
// This allows us to use `unwrap` on `Option` values (because doing makes
// working with Regex matches much nicer) and when compiling in test mode
// (because using it in tests is idiomatic).
#![cfg_attr(all(not(test), feature = "clippy"), warn(result_unwrap_used))]
#![cfg_attr(feature = "clippy", warn(unseparated_literal_suffix))]
#![cfg_attr(feature = "clippy", warn(wrong_pub_self_convention))]

use crate::cfg::load_config;
use crate::notifiers::{new_notifier, NotifierMultiplexerBuilder};
use crate::secrets::{new_secrets, SecretsType};
use crate::utils::{
    get_config_file_path, get_default_config_dir, get_default_log_dir, get_or_create_secret_key,
    GetSecretKeyState,
};
use anyhow::anyhow;
use clap::Parser;
use crossbeam_channel::select;
use log::{debug, error, info, warn};
use std::io::{stdin, stdout, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use you_have_mail_common::backend::BackendError;
use you_have_mail_common::{Account, AccountError, Observer, ObserverBuilder, Secret};

mod cfg;
mod logging;
mod notifiers;
mod secrets;
mod utils;

pub const APP_IDENTIFIER: &str = "dev.lbeernaert.you-have-mail-cli";

const LOG_DIR_DESC: &str = "Directory where the log files will be written";
const CONFIG_DIR_DESC: &str = "Directory where the config files will be written";
const CONFIGURE_ACCOUNTS_DESC:&str = "When used will start an interactive prompt to configure any accounts that do not exist or are logged out";
const DELETE_ACCOUNTS_DESC: &str =
    "Log out and delete any accounts that are not listed in the config file";
const CREATE_CONFIG_DESC: &str = "Create an empty config file if none exists";

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Options {
    #[arg(short='c', long, value_hint = clap::ValueHint::DirPath, help= CONFIG_DIR_DESC)]
    config_dir: Option<PathBuf>,
    #[arg(short='l', long, value_hint = clap::ValueHint::DirPath, help = LOG_DIR_DESC)]
    log_dir: Option<PathBuf>,
    #[arg(long, help=CONFIGURE_ACCOUNTS_DESC)]
    configure_accounts: bool,
    #[arg(long, help=DELETE_ACCOUNTS_DESC)]
    delete_accounts: bool,
    #[arg(long, help=CREATE_CONFIG_DESC)]
    create_config: bool,
}

fn main() -> Result<(), anyhow::Error> {
    let options = Options::parse();
    let config_dir = if let Some(cfg_dir) = options.config_dir {
        if !cfg_dir.is_dir() {
            return Err(anyhow!("Supplied config directory is not a directory"));
        }
        cfg_dir.clone()
    } else {
        get_default_config_dir()?
    };

    let log_dir = if let Some(log_dir) = options.log_dir {
        if !log_dir.is_dir() {
            return Err(anyhow!("Supplied log directory is not a directory"));
        }
        log_dir.clone()
    } else {
        get_default_log_dir()?
    };

    std::fs::create_dir_all(&config_dir)
        .map_err(|e| anyhow!("Failed to create config dir '{config_dir:?}': {e}"))?;
    std::fs::create_dir_all(&log_dir)
        .map_err(|e| anyhow!("Failed to create log dir '{log_dir:?}': {e}"))?;

    logging::init_log(&log_dir)?;

    debug!("-------------------------------------------------------------------------------------");
    info!("Starting You Have Mail CLI");

    let config = load_config(&config_dir, options.create_config)?;

    debug!("Secret store = {:?}", config.secrets);
    debug!("Notifier list= {:?}", config.notifiers);

    if config.secrets == SecretsType::Plain && !config.accept_plain_secrets_insecure {
        let msg = "Plain unencrypted secrets storage, please consent to the risks by setting `accept_plain_secrets_insecure=true` in your config file";
        error!("{msg}");
        return Err(anyhow!(msg));
    }

    if config.notifiers.is_empty() {
        let msg = "No notifiers specified";
        error!("{msg}");
        return Err(anyhow!(msg));
    }

    let notifier = {
        let mut builder = NotifierMultiplexerBuilder::new();
        for n in &config.notifiers {
            let notifier = new_notifier(*n).map_err(|e| {
                error!("{e}");
                e
            })?;
            builder = builder.with_notifier(notifier);
        }

        Arc::new(builder.build())
    };

    let mut secret_store = new_secrets(config.secrets, &config_dir).map_err(|e| {
        error!("{e}");
        e
    })?;
    let encryption_key = get_or_create_secret_key(secret_store.as_mut())?;

    let config_file_path = get_config_file_path(&config_dir);

    let observer_config = match encryption_key {
        GetSecretKeyState::New(key) => {
            if config_file_path.exists() {
                warn!("Existing config file detected but we got a new encryption key, previous state will be lost");
            }
            you_have_mail_common::Config::new(
                key,
                config_file_path,
                Duration::from_secs(config.poll_interval),
            )
            .map_err(|e| {
                error!("{e}");
                e
            })?
        }
        GetSecretKeyState::Existing(key) => {
            you_have_mail_common::Config::create_or_load(key, config_file_path).map_err(|e| {
                error!("{e}");
                e
            })?
        }
    };

    let mut observer = {
        ObserverBuilder::new(notifier, observer_config)
            .default_backends()
            .load_from_config()
            .map_err(|e| {
                error!("{e}");
                e
            })?
    };

    observer
        .set_poll_interval(Duration::from_secs(config.poll_interval))
        .map_err(|e| anyhow!("Failed to set poll interval on observer: {e}"))?;

    if options.delete_accounts {
        delete_accounts(&mut observer, config.account)?;
        return Ok(());
    }

    if options.configure_accounts {
        if let Some(accounts) = config.account {
            configure_accounts(&mut observer, accounts)?
        }
        return Ok(());
    }

    if observer.is_empty() {
        if let Some(accounts) = &config.account {
            if !accounts.is_empty() {
                warn!("No accounts in observer, but found {} account(s) in config file, use --configure-accounts to configure them.", accounts.len());
            }
        }
    } else {
        info!("Detected the following accounts:");
        for (email, account) in observer.accounts() {
            info!(
                "  {} ({}) State: {}",
                email,
                account.backend().name(),
                if account.is_logged_in() {
                    "Logged In"
                } else {
                    "Logged Out/Session Expired"
                }
            )
        }

        if let Some(accounts) = &config.account {
            for account in accounts {
                if observer.get_account(&account.email).is_none() {
                    warn!("Account {} is in config file, but not configured. Use --configure-accounts to configure.", account.email);
                }
            }
        }
    }

    info!(
        "Poll interval {} seconds",
        observer.get_poll_interval().as_secs()
    );

    info!("Starting observer loop - Ctrl+C to Quit");
    let (signal_sender, signal_receiver) = crossbeam_channel::bounded::<()>(0);
    ctrlc::set_handler(move || {
        info!("Received CtrlC signal");
        signal_sender.send(()).expect("failed to send signal");
    })
    .expect("Failed to install ctrl+c handler");

    let timer = crossbeam_channel::tick(observer.get_poll_interval());

    loop {
        observer.poll().expect("Failed to poll");
        select! {
            recv(timer) -> _ => continue,
            recv(signal_receiver) -> _ =>  {
                info!("Exiting");
                return Ok(());
            },
        }
    }
}

fn configure_accounts(observer: &mut Observer, accounts: Vec<cfg::Account>) -> anyhow::Result<()> {
    info!("Checking Config Accounts");

    for account in &accounts {
        let prompt = if let Some(obs_account) = observer.get_account(&account.email) {
            if obs_account.backend().name() != account.backend {
                return Err(anyhow!("Account {} already in observer, but with different backend please remove first.", account.email));
            }
            !obs_account.is_logged_in()
        } else {
            true
        };

        if prompt {
            prompt_account_auth(observer, account)?;
            info!("Account {} added", account.email)
        }
    }

    info!("All accounts configured");
    Ok(())
}

fn prompt_account_auth(observer: &mut Observer, cfg_account: &cfg::Account) -> anyhow::Result<()> {
    let Some(backend) = observer.backend_by_name(&cfg_account.backend) else {
        return Err(anyhow!("Could not locate backed with name '{}'", cfg_account.backend));
    };

    let password = loop {
        let password =
            rpassword::prompt_password(format!("Please type password for {}: ", cfg_account.email))
                .map_err(|_| anyhow!("Failed to read password"))?;
        if password.is_empty() {
            eprintln!("Password can't be empty, please try again");
            continue;
        }
        break Secret::new(password);
    };

    let mut account = Account::new(backend, cfg_account.email.clone(), None);

    match account.login(&password, None) {
        Ok(()) => {}
        Err(AccountError::Backend(BackendError::HVCaptchaRequest(_))) => {
            return Err(anyhow!(
                "Account {} requested captcha validation, this is not supported in CLI:",
                cfg_account.email
            ))?;
        }
        Err(e) => {
            return Err(anyhow!(
                "Failed to login account {}: {e}",
                cfg_account.email
            ))?;
        }
    }

    if account.is_awaiting_totp() {
        for _ in 0..5 {
            let stdin = stdin().lock();
            let mut line_reader = std::io::BufReader::new(stdin);
            print!("Please type TOTP 2FA for {}: ", account.email());
            stdout().flush().expect("Failed to flush stdout");
            let mut line = String::new();
            line_reader
                .read_line(&mut line)
                .expect("Failed to read line");
            let code = line.trim_end_matches('\n');
            if code.is_empty() {
                eprintln!("TOTP 2FA Code can't be empty, please try again");
                continue;
            }

            if let Err(e) = account.submit_totp(code) {
                eprintln!("Failed to submit TOTP code: {e}");
                continue;
            }

            break;
        }

        if account.is_awaiting_totp() {
            return Err(anyhow!("Too many failed TOTP attempts"));
        }
    }

    observer
        .add_account(account)
        .map_err(|e| anyhow!("Failed to add account {}: {e}", cfg_account.email))
}

fn delete_accounts(
    observer: &mut Observer,
    accounts: Option<Vec<cfg::Account>>,
) -> anyhow::Result<()> {
    let accounts_to_delete = if let Some(accounts) = accounts {
        observer
            .accounts()
            .filter(|&(e, _)| accounts.iter().any(|a| a.email == *e))
            .map(|(e, _)| e.clone())
            .collect::<Vec<_>>()
    } else {
        observer
            .accounts()
            .map(|(e, _)| e.clone())
            .collect::<Vec<_>>()
    };

    info!("Found {} account(s) to delete", accounts_to_delete.len());
    for account in accounts_to_delete {
        info!("Logging out and deleting {}", account);
        observer
            .remove_account(&account)
            .map_err(|e| anyhow!("Failed to delete account {}: {e}", account))?;
    }

    Ok(())
}
