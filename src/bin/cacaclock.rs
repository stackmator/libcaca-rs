//! Port of libcaca's `src/cacaclock.c`: text-mode clock display.
//!
//! Renders the current local time with a FIGlet font, centred on the
//! display and refreshed four times a second. Any key quits.
//! `-f/--font` selects the `.flf` file, `-d/--dateformat` the format.
//!
//! The C version formats with `strftime(3)` in the current locale. Rust
//! has no ambient locale, so a small built-in formatter covers the usual
//! specifiers (`%H %M %S %R %T %d %m %Y %y %a %A %b %B %p %j %%` and
//! friends); anything else is copied through literally.

use libcaca::getopt::{Getopt, LongOpt};
use libcaca::{Canvas, Color, Display, EventMask, FigFont};

const VERSION: &str = "0.1";

fn usage(prog: &str) {
    eprintln!("Usage: {prog} [OPTIONS]...");
    eprintln!("Display current time in text mode     (q to quit)");
    eprintln!("Example : {prog} -d '%R'\n");
    eprintln!("Options:");
    eprintln!("  -h, --help\t\t\tThis help");
    eprintln!("  -v, --version\t\t\tVersion of the program");
    eprintln!("  -f, --font=FONT\t\tUse FONT for time display");
    eprintln!("  -d, --dateformat=FORMAT\tUse FORMAT as strftime argument (default %R:%S)");
}

fn version() {
    println!(
        "cacaclock Copyright 2011-2012 Jean-Yves Lamoureux
Internet: <jylam@lnxscene.org> Version: {} (libcaca {}), no build date recorded
",
        VERSION,
        Canvas::version()
    );
    println!("cacaclock, along with its documentation, may be freely copied and distributed.\n");
    println!("The latest version of cacaclock is available from the web site,");
    println!("        http://caca.zoy.org/wiki/libcaca in the libcaca package.\n");
}

/// Local civil time as (hour, min, sec, day, month, year, wday, yday).
fn local_time() -> (i32, i32, i32, i32, i32, i32, i32, i32) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // `libc` is available on every platform where `std` is on.
    let t = now as libc::time_t;
    let tm = unsafe {
        #[cfg(unix)]
        {
            *libc::localtime(&t)
        }
        #[cfg(windows)]
        {
            let mut tm: libc::tm = core::mem::zeroed();
            libc::localtime_s(&mut tm, &t);
            tm
        }
    };
    let (y, m, d) = (tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday);
    (
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
        d,
        m,
        y,
        tm.tm_wday,
        tm.tm_yday + 1,
    )
}

/// Minimal `strftime`, enough for clock formats.
fn strftime(format: &str, t: (i32, i32, i32, i32, i32, i32, i32, i32)) -> String {
    const WEEKDAYS: &[&str] = &[
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    const MONTHS: &[&str] = &[
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let (hh, mm, ss, dd, mo, yy, wd, yd) = t;
    let h12 = if hh % 12 == 0 { 12 } else { hh % 12 };
    let mut out = String::new();
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('H') => out.push_str(&format!("{hh:02}")),
            Some('M') => out.push_str(&format!("{mm:02}")),
            Some('S') => out.push_str(&format!("{ss:02}")),
            Some('R') => out.push_str(&format!("{hh:02}:{mm:02}")),
            Some('T') => out.push_str(&format!("{hh:02}:{mm:02}:{ss:02}")),
            Some('I') => out.push_str(&format!("{h12:02}")),
            Some('k') => out.push_str(&format!("{hh:2}")),
            Some('l') => out.push_str(&format!("{h12:2}")),
            Some('p') => out.push_str(if hh < 12 { "AM" } else { "PM" }),
            Some('P') => out.push_str(if hh < 12 { "am" } else { "pm" }),
            Some('d') => out.push_str(&format!("{dd:02}")),
            Some('e') => out.push_str(&format!("{dd:2}")),
            Some('m') => out.push_str(&format!("{mo:02}")),
            Some('Y') => out.push_str(&format!("{yy:04}")),
            Some('y') => out.push_str(&format!("{:02}", yy % 100)),
            Some('C') => out.push_str(&format!("{:02}", yy / 100)),
            Some('j') => out.push_str(&format!("{yd:03}")),
            Some('a') => out.push_str(&WEEKDAYS[wd as usize % 7][..3]),
            Some('A') => out.push_str(WEEKDAYS[wd as usize % 7]),
            Some('b') | Some('h') => out.push_str(&MONTHS[mo as usize - 1][..3]),
            Some('B') => out.push_str(MONTHS[mo as usize - 1]),
            Some('D') => out.push_str(&format!("{mo:02}/{dd:02}/{:02}", yy % 100)),
            Some('F') => out.push_str(&format!("{yy:04}-{mo:02}-{dd:02}")),
            Some('s') => out.push_str(&format!("{}", now_epoch())),
            Some('%') => out.push('%'),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn run() -> i32 {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv[0].clone();

    let mut font = "/usr/share/figlet/mono12.tlf".to_string();
    let mut format = "%R:%S".to_string();

    let long_options = [
        LongOpt {
            name: "font",
            has_arg: true,
            val: b'f' as i32,
        },
        LongOpt {
            name: "dateformat",
            has_arg: true,
            val: b'd' as i32,
        },
        LongOpt {
            name: "help",
            has_arg: false,
            val: b'h' as i32,
        },
        LongOpt {
            name: "version",
            has_arg: false,
            val: b'v' as i32,
        },
    ];
    let mut g = Getopt::new(argv.clone());
    loop {
        let c = g.next("f:d:hv", &long_options, None);
        if c == -1 {
            break;
        }
        match c {
            x if x == b'f' as i32 => font = g.optarg().unwrap_or("").to_string(),
            x if x == b'd' as i32 => format = g.optarg().unwrap_or("").to_string(),
            x if x == b'h' as i32 => {
                usage(&prog);
                return 0;
            }
            x if x == b'v' as i32 => {
                version();
                return 0;
            }
            _ => return 1,
        }
    }

    let cv = match Canvas::new(0, 0) {
        Ok(cv) => cv,
        Err(_) => {
            eprintln!("{prog}: unable to initialise libcaca");
            return 1;
        }
    };
    let figcv = match Canvas::new(0, 0) {
        Ok(cv) => cv,
        Err(_) => {
            eprintln!("{prog}: unable to initialise libcaca");
            return 1;
        }
    };
    let mut fig = match FigFont::load(&font) {
        Ok(fig) => fig,
        Err(_) => {
            eprintln!("Could not open font");
            return -1;
        }
    };

    let mut dp = match Display::new(cv) {
        Ok(dp) => dp,
        Err(_) => {
            println!("Can't open window. CACA_DRIVER problem ?");
            return -1;
        }
    };

    let mut figcv = figcv;
    let _ = dp
        .canvas_mut()
        .set_color_ansi(Color::Default, Color::Default);
    dp.canvas_mut().clear();

    loop {
        // Any event at all ends the program, like the C version.
        if dp
            .get_event(EventMask::KEY_PRESS | EventMask::QUIT, 1)
            .is_some()
        {
            break;
        }

        let date = strftime(&format, local_time());

        // The figfont API renders at 0,0, so compose on a spare canvas
        // and blit it centred, exactly like the C version.
        dp.canvas_mut().clear();
        figcv.clear();
        for ch in date.chars() {
            let _ = fig.put_char(&mut figcv, ch as u32);
        }
        let _ = fig.flush(&mut figcv);

        let (w, h) = (dp.canvas().width(), dp.canvas().height());
        let (fw, fh) = (figcv.width(), figcv.height());
        let (x, y) = (w / 2 - fw / 2, h / 2 - fh / 2);
        let _ = dp.canvas_mut().blit(x, y, &figcv, None);
        let _ = dp.refresh();
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    0
}

fn main() {
    std::process::exit(run());
}
