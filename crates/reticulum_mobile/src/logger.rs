use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::event_bus::EventBus;
use crate::types::{LogLevel, NodeEvent};

fn log_record_level_to_udl(level: Level) -> LogLevel {
    match level {
        Level::Trace => LogLevel::Trace {},
        Level::Debug => LogLevel::Debug {},
        Level::Info => LogLevel::Info {},
        Level::Warn => LogLevel::Warn {},
        Level::Error => LogLevel::Error {},
    }
}

fn log_level_to_filter(level: &LogLevel) -> LevelFilter {
    match level {
        LogLevel::Trace {} => LevelFilter::Trace,
        LogLevel::Debug {} => LevelFilter::Debug,
        LogLevel::Info {} => LevelFilter::Info,
        LogLevel::Warn {} => LevelFilter::Warn,
        LogLevel::Error {} => LevelFilter::Error,
    }
}

pub struct NodeLogger {
    level: AtomicU8,
    bus: Mutex<Option<EventBus>>,
}

impl NodeLogger {
    pub fn global() -> &'static NodeLogger {
        static LOGGER: OnceLock<NodeLogger> = OnceLock::new();
        LOGGER.get_or_init(|| NodeLogger {
            level: AtomicU8::new(LevelFilter::Info as u8),
            bus: Mutex::new(None),
        })
    }

    pub fn install() {
        let logger = Self::global();
        let _ = log::set_logger(logger);
        log::set_max_level(LevelFilter::Info);
    }

    pub fn set_level(&self, level: LogLevel) {
        let filter = log_level_to_filter(&level);
        self.level.store(filter as u8, Ordering::Relaxed);
        log::set_max_level(filter);
    }

    pub fn set_bus(&self, bus: Option<EventBus>) {
        if let Ok(mut guard) = self.bus.lock() {
            *guard = bus;
        }
    }

    fn current_level(&self) -> LevelFilter {
        match self.level.load(Ordering::Relaxed) {
            x if x == LevelFilter::Trace as u8 => LevelFilter::Trace,
            x if x == LevelFilter::Debug as u8 => LevelFilter::Debug,
            x if x == LevelFilter::Info as u8 => LevelFilter::Info,
            x if x == LevelFilter::Warn as u8 => LevelFilter::Warn,
            x if x == LevelFilter::Error as u8 => LevelFilter::Error,
            _ => LevelFilter::Info,
        }
    }
}

impl Log for NodeLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        let max = self
            .current_level()
            .to_level()
            .unwrap_or(log::Level::Info);
        metadata.level() <= max
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let message = format!("{}", record.args());

        if let Ok(guard) = self.bus.lock() {
            if let Some(bus) = guard.clone() {
                bus.emit(NodeEvent::Log {
                    level: log_record_level_to_udl(record.level()),
                    message: message.clone(),
                });
            }
        }

        // Always write to stderr for native troubleshooting.
        eprintln!("[{}] {}", record.level(), message);
    }

    fn flush(&self) {}
}

