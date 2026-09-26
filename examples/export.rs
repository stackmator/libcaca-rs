//! Port of libcaca's `examples/export.c`.
//!
//! Without a file argument it builds the classic libcaca showcase canvas (a
//! dithered gradient plus text, colours and styles) and exports it to stdout.
//! With a file argument it imports the file and exports it instead.
//!
//! Usage: `export [file] <format>` — run `export` with no valid format to
//! list the supported formats.

use std::io::Write;
use std::path::Path;

use libcaca::{Attr, Canvas, Color, Dither, Style};

const WIDTH: i32 = 80;
const HEIGHT: i32 = 32;

const DESCRIPTIONS: &[(&str, &str)] = &[
    ("caca", "native libcaca format"),
    ("ansi", "ANSI"),
    ("utf8", "UTF-8 with ANSI escape codes"),
    ("utf8cr", "UTF-8 with ANSI escape codes and MS-DOS \\r"),
    ("html", "HTML"),
    ("html3", "backwards-compatible HTML"),
    ("html5", "HTML5"),
    ("bbfr", "BBCode (French)"),
    ("irc", "IRC with mIRC colours"),
    ("ps", "PostScript document"),
    ("svg", "SVG vector image"),
    ("tga", "TGA image"),
    ("troff", "troff source"),
];

fn usage(prog: &str) -> ! {
    eprintln!("{}: wrong argument count", prog);
    eprintln!("usage: {} [file] <format>", prog);
    eprintln!("where <format> is one of:");
    for (name, desc) in DESCRIPTIONS {
        eprintln!(" \"{}\" ({})", name, desc);
    }
    std::process::exit(-1);
}

fn main() -> libcaca::Result<()> {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv.first().cloned().unwrap_or_else(|| "export".to_string());

    let (file, format) = match argv.len() {
        2 => (None, argv[1].clone()),
        3 => (Some(argv[1].clone()), argv[2].clone()),
        _ => usage(&prog),
    };

    if libcaca::codec::export_list()
        .iter()
        .all(|f| !f.eq_ignore_ascii_case(&format))
    {
        eprintln!("{}: unknown format `{}'", prog, format);
        eprintln!("please use one of:");
        for (name, desc) in DESCRIPTIONS {
            eprintln!(" \"{}\" ({})", name, desc);
        }
        std::process::exit(-1);
    }

    let cv = if let Some(file) = file {
        let mut cv = Canvas::new(0, 0)?;
        if cv.import_from_file(Path::new(&file), "").is_err() {
            eprintln!("{}: `{}` has unknown format", prog, file);
            std::process::exit(-1);
        }
        cv
    } else {
        let mut cv = Canvas::new(WIDTH, HEIGHT)?;

        let mut pixels = Vec::with_capacity(256 * 256 * 4);
        for y in 0..256i32 {
            for x in 0..256i32 {
                let r = x as u32;
                let g = ((255 - y + x) / 2) as u32;
                let b = (y * (255 - x) / 256) as u32;
                pixels.extend_from_slice(&((r << 16) | (g << 8) | b).to_le_bytes());
            }
        }

        // NB: the C demo passes bpp=256 here, which this port correctly
        // rejects; the source image is 256x256 32-bit RGBA.
        let mut dither = Dither::new(
            256,
            256,
            32,
            4 * 256,
            0x00ff0000,
            0x0000ff00,
            0x000000ff,
            0,
        )?;
        if format == "ansi" || format == "utf8" {
            dither.set_charset("shades")?;
        }
        let (w, h) = (cv.width(), cv.height());
        dither.dither_bitmap(&mut cv, 0, 0, w, h, &pixels)?;

        cv.set_color_ansi(Color::White, Color::Black)?;
        cv.draw_thin_box(0, 0, WIDTH - 1, HEIGHT - 1);

        cv.set_color_ansi(Color::Black, Color::White)?;
        cv.fill_ellipse(cv.width() / 2, HEIGHT / 2, WIDTH / 4, HEIGHT / 4, b' ' as u32);

        cv.set_color_ansi(Color::LightGray, Color::Black)?;
        cv.put_str(WIDTH / 2 - 12, HEIGHT / 2 - 6, "   lightgray on black   ");
        cv.set_color_ansi(Color::Default, Color::Transparent)?;
        cv.put_str(WIDTH / 2 - 12, HEIGHT / 2 - 5, " default on transparent ");
        cv.set_color_ansi(Color::Black, Color::White)?;
        cv.put_str(WIDTH / 2 - 12, HEIGHT / 2 - 4, "     black on white     ");

        cv.set_color_ansi(Color::Black, Color::White)?;
        cv.put_str(WIDTH / 2 - 8, HEIGHT / 2 - 3, "[<><><><> <>--<>]");
        cv.put_str(WIDTH / 2 - 8, HEIGHT / 2 - 2, "[ドラゴン ボーレ]");
        cv.put_str(WIDTH / 2 - 7, HEIGHT / 2 + 2, "äβç ░▒▓█▓▒░ ΔЗҒ");
        cv.put_str(WIDTH / 2 - 5, HEIGHT / 2 + 4, "(\") \\o/ <&>");

        cv.set_attr(Attr::from_raw(Style::BOLD.bits() as u32));
        cv.put_str(WIDTH / 2 - 16, HEIGHT / 2 + 3, "Bold");
        cv.set_attr(Attr::from_raw(Style::BLINK.bits() as u32));
        cv.put_str(WIDTH / 2 - 9, HEIGHT / 2 + 3, "Blink");
        cv.set_attr(Attr::from_raw(Style::ITALICS.bits() as u32));
        cv.put_str(WIDTH / 2 - 1, HEIGHT / 2 + 3, "Italics");
        cv.set_attr(Attr::from_raw(Style::UNDERLINE.bits() as u32));
        cv.put_str(WIDTH / 2 + 8, HEIGHT / 2 + 3, "Underline");
        cv.set_attr(Attr::from_raw(0));

        cv.set_color_ansi(Color::White, Color::LightBlue)?;
        cv.put_str(WIDTH / 2 - 7, HEIGHT / 2, "    LIBCACA    ");

        for x in 0..16u16 {
            cv.set_color_argb(0xff00 | x, 0xf00f | (x << 4));
            cv.put_char(WIDTH / 2 - 7 + x as i32, HEIGHT / 2 + 6, b'#' as u32);
        }

        cv
    };

    let data = cv.export_to_memory(&format)?;
    let mut out = std::io::stdout();
    let _ = out.write_all(&data);
    let _ = out.flush();

    Ok(())
}
