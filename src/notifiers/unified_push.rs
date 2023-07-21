use crate::notifiers::Notifier;
use anyhow::anyhow;
use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use log::{debug, error};
use serde::Deserialize;
use std::time::Duration;
use ureq::Error;
use you_have_mail_common::backend::EmailInfo;
use you_have_mail_common::{ExposeSecret, Notification, Secret};

#[derive(Debug, Deserialize)]
/// Configuration for unified push endpoint
pub struct UnifiedPushConfig {
    pub name: String,
    pub url: String,
    pub auth_token: Option<String>,
}

impl UnifiedPushConfig {
    pub fn into_notifier(self) -> anyhow::Result<Box<dyn Notifier>> {
        let notifier = UnifiedPushNotifier::new(self)?;
        Ok(Box::new(notifier))
    }
}

/// Send notifications to a unified push distributor (tested with ntfy instances).
struct UnifiedPushNotifier {
    name: String,
    sender: Sender<UPNotification>,
}

enum UPNotification {
    NewEmail(String, String, Vec<EmailInfo>),
    LoggedOut(String),
    AccountError(String, String),
    ConfigError(String),
    Error(String),
}

impl Notifier for UnifiedPushNotifier {
    fn notify(&self, notification: &Notification) {
        let up_notification = match notification {
            Notification::NewEmail {
                account,
                backend,
                emails,
            } => {
                UPNotification::NewEmail(account.to_string(), backend.to_string(), emails.to_vec())
            }
            Notification::AccountLoggedOut(email) => UPNotification::LoggedOut(email.to_string()),
            Notification::AccountError(email, error) => {
                UPNotification::AccountError(email.to_string(), error.to_string())
            }
            Notification::ConfigError(e) => UPNotification::ConfigError(e.to_string()),
            Notification::Error(e) => UPNotification::Error(e.clone()),
            _ => {
                return;
            }
        };

        if let Err(e) = self.sender.send(up_notification) {
            error!("Failed to sent notification to thread ({}): {e}", self.name);
        }
    }
}

impl UnifiedPushNotifier {
    pub fn new(config: UnifiedPushConfig) -> anyhow::Result<Self> {
        let agent = ureq::builder()
            .timeout_connect(Duration::from_secs(60))
            .timeout(Duration::from_secs(120))
            .max_idle_connections(0)
            .build();
        let (sender, receiver) = crossbeam_channel::bounded(20);
        let thread_state = ThreadState {
            agent,
            receiver,
            server_url: config.url,
            name: config.name.clone(),
            auth_token: config.auth_token.map(Secret::new),
        };
        std::thread::Builder::new()
            .name("unified-push".to_string())
            .spawn(move || ThreadState::thread_loop(thread_state))
            .map_err(|e| anyhow!("Failed to spawn unified push thread ({}): {e}", config.name))?;

        Ok(Self {
            sender,
            name: config.name,
        })
    }
}

struct ThreadState {
    name: String,
    agent: ureq::Agent,
    receiver: Receiver<UPNotification>,
    server_url: String,
    auth_token: Option<Secret<String>>,
}

impl ThreadState {
    fn thread_loop(state: ThreadState) {
        debug!("Starting unified push thread");
        while let Ok(notification) = state.receiver.recv() {
            match notification {
                UPNotification::NewEmail(account, _backend, emails) => {
                    let title = format!("{account} has {} new message(s))", emails.len());
                    let mut body = String::new();
                    for email in emails {
                        body.push_str(&format!("**{}**: {}\n", email.sender, email.subject))
                    }
                    state.info_notification(title, Some(body));
                }
                UPNotification::LoggedOut(email) => {
                    state.info_notification(format!("{email} logged out or session expired"), None);
                }
                UPNotification::AccountError(email, e) => {
                    let title = format!("{email} encountered an error");
                    state.error_notification(title, Some(e));
                }
                UPNotification::ConfigError(e) => {
                    state.error_notification("Server Config Error".to_string(), Some(e));
                }
                UPNotification::Error(e) => {
                    state.error_notification("Server Error".to_string(), Some(e));
                }
            }
        }
        debug!("Exiting unified push thread")
    }

    fn new_request(&self) -> ureq::Request {
        let request = self
            .agent
            .request("POST", &self.server_url)
            .set("X-UnifiedPush", "1");
        if let Some(token) = &self.auth_token {
            request.set(
                "authorization",
                &format!("Bearer {}", token.expose_secret()),
            )
        } else {
            request
        }
    }

    fn info_notification(&self, title: String, body: Option<String>) {
        let request = self.new_request();
        self.send(request, title, body)
    }

    fn error_notification(&self, title: String, body: Option<String>) {
        let request = self.new_request().set("X-Tags", "exclamation");
        self.send(request, title, body)
    }

    fn send(&self, request: ureq::Request, title: String, body: Option<String>) {
        match if let Some(body) = body {
            request.set("X-Title", &title).send_string(&body)
        } else {
            request.send_string(&title)
        } {
            Ok(_) => {}
            Err(e) => match e {
                Error::Status(code, response) => {
                    let response_body = match response.into_string() {
                        Ok(s) => s,
                        Err(_) => "Failed to get response body".to_string(),
                    };
                    error!(
                        "Failed to sent unified push request ({}): {} - {}",
                        self.name, code, response_body
                    );
                }
                Error::Transport(e) => {
                    error!("Failed to sent unified push request ({}): {e}", self.name,);
                }
            },
        };
    }
}
