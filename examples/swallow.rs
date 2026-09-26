//! Port of libcaca's `examples/swallow.c`.
//!
//! A libcaca multiplexer: runs four child programs emitting the native `caca`
//! stream (e.g. other examples with `CACA_DRIVER=raw`) and tiles them in
//! quadrants. Usage: `swallow <cmd1> <cmd2> <cmd3> <cmd4>`. Any key quits.
//!
//! Child I/O uses blocking reads like the C version; all four pipes must
//! produce output for their quadrant to update.

use std::io::Read;
use std::process::{Child, Command, Stdio};

use libcaca::{Canvas, Color, Display, Driver, EventMask};

fn spawn_child(cmd: &str, w: i32, h: i32) -> std::io::Result<Child> {
    let geometry = format!("{}x{}", w, h);
    #[cfg(unix)]
    {
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .env("CACA_DRIVER", "raw")
            .env("CACA_GEOMETRY", geometry)
            .stdout(Stdio::piped())
            .spawn()
    }
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", cmd])
            .env("CACA_DRIVER", "raw")
            .env("CACA_GEOMETRY", geometry)
            .stdout(Stdio::piped())
            .spawn()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (cmd, geometry);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no shell available",
        ))
    }
}

fn main() -> libcaca::Result<()> {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 5 {
        eprintln!("usage: {} <cmd1> <cmd2> <cmd3> <cmd4>", argv[0]);
        std::process::exit(1);
    }

    // NB: the C version starts from a 0x0 canvas that the driver sizes; use
    // 80x24 so the headless (null) driver has a real geometry to tile.
    let mut dp = Display::new(Canvas::new(80, 24)?)?;
    let mut app = Canvas::new(0, 0)?;

    let (w, h) = {
        let cv = dp.canvas();
        ((cv.width() - 4) / 2, (cv.height() - 6) / 2)
    };
    if w < 0 || h < 0 {
        std::process::exit(1);
    }

    {
        let cv = dp.canvas_mut();
        cv.set_color_ansi(Color::White, Color::Blue)?;
        let cw = cv.width();
        cv.draw_line(0, 0, cw - 1, 0, b' ' as u32);
        cv.printf(cw / 2 - 10, 0, format_args!("libcaca multiplexer"));
    }

    let mut children: Vec<Child> = Vec::new();
    let mut pipes = Vec::new();
    for i in 0..4 {
        let mut child = spawn_child(&argv[i + 1], w, h).map_err(|_| {
            eprintln!("{}: could not spawn `{}`", argv[0], argv[i + 1]);
            libcaca::CacaError::Invalid
        })?;
        pipes.push(child.stdout.take());
        children.push(child);
        dp.canvas_mut().printf(
            (w + 2) * (i as i32 / 2) + 1,
            (h + 2) * ((i as i32 % 2) + 1),
            format_args!("{}", argv[i + 1]),
        );
    }

    let interactive = matches!(dp.driver(), Driver::Terminal | Driver::Win32);
    let mut bufs: Vec<Vec<u8>> = vec![Vec::new(); 4];
    let mut done = [false; 4];

    loop {
        if let Some(ev) = dp.get_event(EventMask::ANY, 0) {
            if ev.event_type() == EventMask::KEY_PRESS {
                break;
            }
        }

        let mut all_done = true;
        for i in 0..4 {
            if done[i] {
                continue;
            }
            all_done = false;

            match app.import_from_memory(&bufs[i], "caca") {
                Ok(n) if n > 0 => {
                    bufs[i].drain(..n);
                    let cv = dp.canvas_mut();
                    cv.blit(
                        (w + 2) * (i as i32 / 2) + 1,
                        (h + 2) * (i as i32 % 2) + 2,
                        &app,
                        None,
                    )?;
                    dp.refresh()?;
                }
                Ok(_) => {
                    let mut tmp = [0u8; 128];
                    match pipes[i].as_mut().map(|p| p.read(&mut tmp)) {
                        Some(Ok(0)) | None => done[i] = true,
                        Some(Ok(n)) => bufs[i].extend_from_slice(&tmp[..n]),
                        Some(Err(_)) => done[i] = true,
                    }
                }
                Err(_) => {
                    eprintln!("{}: corrupted input {}", argv[0], i);
                    std::process::exit(-1);
                }
            }
        }

        if all_done || !interactive {
            // Exhausted children, or a single headless pass: exit.
            break;
        }
    }

    for mut child in children {
        let _ = child.kill();
        let _ = child.wait();
    }

    Ok(())
}
