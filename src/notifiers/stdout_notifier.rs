use crate::notifiers::Notifier;
use you_have_mail_common::Notification;

/// Write notifications to stdout.
#[derive(Debug, Copy, Clone)]
pub struct StdOutNotifier {}

impl Notifier for StdOutNotifier {
    fn notify(&self, notification: &Notification) {
        match notification {
            Notification::NewEmail {
                account,
                backend,
                emails,
            } => {
                println!(
                    "Account {account} ({backend}) received {} new email(s)",
                    emails.len()
                );
                for e in emails.iter() {
                    println!("    Sender={} Subject={}", e.sender, e.subject);
                }
            }
            Notification::AccountLoggedOut(account) => {
                println!("Account {account} Logged out or Session Expired");
            }
            Notification::AccountError(account, error) => {
                println!("Account {account} ran into an error: {}", error)
            }
            Notification::ProxyApplied(_, _) => {}
            Notification::ConfigError(error) => {
                println!("Configuration error: {}", error);
            }
            Notification::Error(error) => {
                println!("An error occurred: {}", error);
            }
            _ => {}
        }
    }
}
