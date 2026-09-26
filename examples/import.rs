//! Port of libcaca's `examples/import.c`.
//!
//! Usage: `import <filename> [<format>]` — imports the file and shows it.
//! Press any key to quit when running interactively.

use std::path::Path;

use libcaca::{Canvas, Display, Driver, EventMask};

fn main() -> libcaca::Result<()> {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 2 {
        eprintln!("{}: missing argument (filename).", argv[0]);
        eprintln!("usage: {} <filename> [<format>]", argv[0]);
        std::process::exit(1);
    }

    let mut cv = Canvas::new(0, 0)?;
    let format = argv.get(2).map(|s| s.as_str()).unwrap_or("");
    if cv.import_from_file(Path::new(&argv[1]), format).is_err() {
        eprintln!("{}: could not open `{}`.", argv[0], argv[1]);
        std::process::exit(1);
    }

    let mut dp = Display::new(cv)?;
    dp.refresh()?;

    if matches!(dp.driver(), Driver::Terminal | Driver::Win32) {
        loop {
            if dp
                .get_event(EventMask::KEY_PRESS | EventMask::QUIT, -1)
                .is_some()
            {
                break;
            }
        }
    }

    Ok(())
}
