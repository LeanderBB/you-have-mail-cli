//! Collection of notifier implementations.
use serde::Deserialize;
use you_have_mail_common::Notification;
use you_have_mail_common::Notifier as YHMNotifier;

mod stdout_notifier;

pub trait Notifier: Send + Sync {
    fn notify(&self, notification: &Notification);
}

pub struct NotifierMultiplexer {
    notifiers: Vec<Box<dyn Notifier>>,
}

#[derive(Default)]
pub struct NotifierMultiplexerBuilder {
    notifiers: Vec<Box<dyn Notifier>>,
}

impl NotifierMultiplexerBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_notifier(mut self, notifier: Box<dyn Notifier>) -> Self {
        self.notifiers.push(notifier);
        self
    }

    pub fn build(self) -> NotifierMultiplexer {
        NotifierMultiplexer {
            notifiers: self.notifiers,
        }
    }
}

impl YHMNotifier for NotifierMultiplexer {
    fn notify(&self, notification: Notification) {
        for notifier in &self.notifiers {
            notifier.notify(&notification)
        }
    }
}

#[derive(Debug, Deserialize, Copy, Clone, Eq, PartialEq)]
pub enum NotifiersType {
    StdOut,
}

pub fn new_notifier(t: NotifiersType) -> Result<Box<dyn Notifier>, anyhow::Error> {
    match t {
        NotifiersType::StdOut => Ok(Box::new(stdout_notifier::StdOutNotifier {})),
    }
}
