use std::sync::LazyLock;

use env_logger::fmt::{ConfigurableFormat, Formatter};
use log::{Level, LevelFilter, Record};

// Notes on logging
//
// Problems that stop the program are on the error level
// Problems that cause partial results are on the warn level
// Normal operations and info are on the info level
// Description of errors are on the debug level

struct Format {
    low: ConfigurableFormat,
    high: ConfigurableFormat
}

impl Format {
    fn format(
        &self,
        formatter: &mut Formatter,
        record: &Record
    ) -> std::io::Result<()> {
        if record.level() <= Level::Info {
            self.high.format(formatter, record)
        } else {
            self.low.format(formatter, record)
        }
    }
}

static FORMAT: LazyLock<Format> = LazyLock::new(|| {
    let mut low = ConfigurableFormat::default();
    low.timestamp(None)
        .file(true)
        .line_number(true)
        .target(false);
    let mut high = ConfigurableFormat::default();
    high.timestamp(None)
        .target(false);
    Format { low, high }
});

pub fn init(verbose: bool) {
    let filter = match verbose {
        true => LevelFilter::Trace,
        false => LevelFilter::Info
    };
    env_logger::builder()
        .filter_level(LevelFilter::Off)
        .filter_module("nhentai_downloader", filter)
        .format(|f, r| FORMAT.format(f, r))
        .init();
}
