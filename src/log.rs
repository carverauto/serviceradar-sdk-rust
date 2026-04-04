use crate::host;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Logger;

pub const LOG: Logger = Logger;

impl Logger {
    pub fn debug(&self, msg: impl AsRef<str>) {
        log_with_level(LogLevel::Debug, msg.as_ref());
    }

    pub fn info(&self, msg: impl AsRef<str>) {
        log_with_level(LogLevel::Info, msg.as_ref());
    }

    pub fn warn(&self, msg: impl AsRef<str>) {
        log_with_level(LogLevel::Warn, msg.as_ref());
    }

    pub fn error(&self, msg: impl AsRef<str>) {
        log_with_level(LogLevel::Error, msg.as_ref());
    }
}

fn log_with_level(level: LogLevel, msg: &str) {
    if msg.is_empty() {
        return;
    }

    host::log(level as u32, msg.as_bytes());
}

#[cfg(test)]
mod tests;
