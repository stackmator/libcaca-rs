//! Port of libcaca's `examples/import.c`.
//!
//! Imports a file (ANSI art, UTF-8, plain text, `.caca` or `.bin` —
//! autodetected unless a format is given) into a canvas and shows it until
//! a keypress. Usage: `import <filename> [<format>]`.

use std::path::Path;

use libcaca::{Canvas, Display, EventMask};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("{}: missing argument (filename).", args[0]);
        eprintln!("usage: {} <filename> [<format>]", args[0]);
        std::process::exit(1);
    }

    let mut cv = match Canvas::new(0, 0) {
        Ok(cv) => cv,
        Err(_) => {
            println!("Can't create canvas");
            std::process::exit(-1);
        }
    };

    let format = if args.len() >= 3 {
        args[2].as_str()
    } else {
        ""
    };
    if cv.import_from_file(Path::new(&args[1]), format).is_err() {
        eprintln!("{}: could not open `{}`.", args[0], args[1]);
        std::process::exit(1);
    }

    let mut dp = match Display::new(cv) {
        Ok(dp) => dp,
        Err(_) => {
            println!("Can't create display");
            std::process::exit(-1);
        }
    };

    let _ = dp.refresh();
    dp.get_event(EventMask::KEY_PRESS, -1);
}
