//! Port of libcaca's `src/cacaplay.c`: caca file player.
//!
//! Streams a `.caca` animation from a file (or standard input with `-`),
//! importing frame by frame and blitting each onto the display. Any key
//! stops the stream; a final keypress is then awaited before exiting.

use std::io::Read;

use libcaca::{Canvas, Display, Event, EventMask};

fn run() -> i32 {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv[0].clone();

    // Use stdin for no argument or `-`, otherwise open the file.
    let mut input: Box<dyn Read> = if argv.len() < 2 || argv[1] == "-" {
        Box::new(std::io::stdin())
    } else {
        match std::fs::File::open(&argv[1]) {
            Ok(f) => Box::new(f),
            Err(_) => {
                eprintln!("{prog}: could not open `{}`.", argv[1]);
                return 1;
            }
        }
    };

    let cv = match Canvas::new(0, 0) {
        Ok(cv) => cv,
        Err(_) => {
            // Kept verbatim, typo included.
            println!("Can't created canvas");
            return -1;
        }
    };
    let mut app = match Canvas::new(0, 0) {
        Ok(cv) => cv,
        Err(_) => {
            println!("Can't created canvas");
            return -1;
        }
    };
    let mut dp = match Display::new(cv) {
        Ok(dp) => dp,
        Err(_) => {
            println!("Can't create display");
            return -1;
        }
    };

    let mut buf: Vec<u8> = Vec::new();
    let mut total = 0usize;
    let mut bytes = 0usize;

    loop {
        let mut quit = false;
        if let Some(Event::KeyPress(_)) = dp.get_event(EventMask::ANY, 0) {
            break;
        }

        if bytes == 0 {
            let mut byte = [0u8; 1];
            match input.read(&mut byte) {
                Err(_) => {
                    eprintln!("{prog}: read error");
                    return -1;
                }
                Ok(0) => {
                    // End of input: drain below, then leave.
                    quit = true;
                }
                Ok(_) => {
                    buf.extend_from_slice(&byte);
                    total += 1;
                }
            }
        }

        match app.import_from_memory(&buf[..total], "caca") {
            Ok(used) => {
                bytes = used;
                if bytes > 0 {
                    buf.drain(..bytes);
                    total -= bytes;
                    let _ = dp.canvas_mut().blit(0, 0, &app, None);
                    let _ = dp.refresh();
                }
            }
            Err(_) => {
                eprintln!("{prog}: corrupted caca file");
                break;
            }
        }

        if quit {
            break;
        }
    }

    dp.get_event(EventMask::KEY_PRESS, -1);

    0
}

fn main() {
    std::process::exit(run());
}
