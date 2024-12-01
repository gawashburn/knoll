use log::{set_boxed_logger, set_max_level, Log, SetLoggerError};
use simplelog::SharedLogger;
use std::ops::Deref;
use std::sync::{Arc, RwLock};

/// An indirect logger is an implementation of log::Log that delegates
/// all calls to a simplelog::SharedLogger.  This allows the logger to be
/// updated at runtime.  
///
/// This is not really necessary for ordinary operation, but is useful in
/// testing where we may invoke the knoll command multiple times in the
/// same process lifetime.
#[derive(Clone)]
pub struct IndirectLogger {
    logger: Arc<RwLock<Box<dyn SharedLogger>>>,
}

impl IndirectLogger {
    /// Update this IndirectLogger to make use of the new logger in
    /// subsequent calls to log::Log functions.
    pub fn update(&self, logger: Box<dyn SharedLogger>) {
        set_max_level(logger.level());
        *self.logger.write().unwrap() = logger;
    }

    /// Construct a new IndirectLogger that delegates to the given logger.
    pub fn new(logger: Box<dyn SharedLogger>) -> Self {
        IndirectLogger {
            logger: Arc::new(RwLock::new(logger)),
        }
    }

    /// Initialize the global logger with the given logger wrapped by
    /// an IndirectLogger.  This function returns a clone of IndirectLogger
    /// that can be used to update the logger at a later time.
    pub fn init(logger: Box<dyn SharedLogger>) -> Result<Self, SetLoggerError> {
        set_max_level(logger.deref().level());
        let indirect_logger = IndirectLogger::new(logger);
        set_boxed_logger(Box::new(indirect_logger.clone()))?;
        Ok(indirect_logger)
    }
}

impl Log for IndirectLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.logger.read().unwrap().deref().enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        self.logger.read().unwrap().deref().log(record)
    }

    fn flush(&self) {
        self.logger.read().unwrap().deref().flush()
    }
}
