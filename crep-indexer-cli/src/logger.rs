use std::fs::File;
use std::sync::Mutex;

use chrono::Local;
use log::LevelFilter;
use log::SetLoggerError;
use std::io::Write;

struct FileLogger {
    file: Mutex<File>,
    filter: LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.filter
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let mut file = self.file.lock().unwrap();
            writeln!(
                file,
                "[{}:{}] {}",
                record.level(),
                Local::now().format("%H:%M:%S%.3f"),
                record.args()
            )
            .unwrap();
        }
    }

    fn flush(&self) {
        self.file.lock().unwrap().flush().unwrap();
    }
}

impl FileLogger {
    fn new(file: File, filter: LevelFilter) -> Self {
        Self {
            file: Mutex::new(file),
            filter,
        }
    }
}

pub fn init_file_logger(
    file_name: &str,
    filter: LevelFilter,
) -> Result<(), SetLoggerError> {
    log::set_max_level(filter);
    log::set_boxed_logger(Box::new(FileLogger::new(
        File::create(file_name).unwrap(),
        filter,
    )))
}
