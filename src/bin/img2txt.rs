//! Port of libcaca's `src/img2txt.c`: image to text converter.
//!
//! Converts an image file to any text-based export format and writes it to
//! standard output. Usage, options and messages match the C version; image
//! decoding goes through the `import` feature's [`Image`](libcaca::Image)
//! (the C build without Imlib2 only read BMP — this port always decodes
//! every format the `image` crate supports, so the Imlib2 note is gone).
//!
//! Requires the `import` cargo feature.

use std::io::Write;

use libcaca::getopt::{Getopt, LongOpt};
use libcaca::{Canvas, Color, Dither, Image};

fn usage(prog: &str) {
    eprintln!("Usage: {prog} [OPTIONS]... <IMAGE>");
    eprintln!("Convert IMAGE to any text based available format.");
    eprintln!("Example : {prog} -W 80 -f ansi ./caca.png\n");
    eprintln!("Options:");
    eprintln!("  -h, --help\t\t\tThis help");
    eprintln!("  -v, --version\t\t\tVersion of the program");
    eprintln!("  -W, --width=WIDTH\t\tWidth of resulting image");
    eprintln!("  -H, --height=HEIGHT\t\tHeight of resulting image");
    eprintln!("  -x, --font-width=WIDTH\t\tWidth of output font");
    eprintln!("  -y, --font-height=HEIGHT\t\tHeight of output font");
    eprintln!("  -b, --brightness=BRIGHTNESS\tBrightness of resulting image");
    eprintln!("  -c, --contrast=CONTRAST\tContrast of resulting image");
    eprintln!("  -g, --gamma=GAMMA\t\tGamma of resulting image");
    eprintln!("  -d, --dither=DITHER\t\tDithering algorithm to use :");
    let algos = Dither::algorithm_list();
    let mut i = 0;
    while i + 1 < algos.len() {
        eprintln!("\t\t\t{}: {}", algos[i], algos[i + 1]);
        i += 2;
    }

    eprintln!("  -f, --format=FORMAT\t\tFormat of the resulting image :");
    for format in libcaca::codec::export_list() {
        eprintln!("\t\t\t{format}");
    }
}

fn version() {
    println!(
        "img2txt Copyright 2006-2007 Sam Hocevar and Jean-Yves Lamoureux
Internet: <sam@hocevar.net> <jylam@lnxscene.org> Version: {}
(Rust port; no build date is recorded.)
",
        Canvas::version()
    );
    println!("img2txt, along with its documentation, may be freely copied and distributed.\n");
    println!("The latest version of img2txt is available from the web site,");
    println!("        http://caca.zoy.org/wiki/libcaca in the libcaca package.\n");
}

fn run() -> i32 {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv[0].clone();

    let mut cols = 0i32;
    let mut lines = 0i32;
    let mut font_width = 6i32;
    let mut font_height = 10i32;
    let mut format = "ansi".to_string();
    let mut dither: Option<String> = None;
    let mut gamma = -1.0f32;
    let mut brightness = -1.0f32;
    let mut contrast = -1.0f32;

    if argv.len() < 2 {
        eprintln!("{prog}: wrong argument count");
        usage(&prog);
        return 1;
    }

    let long_options = [
        LongOpt {
            name: "width",
            has_arg: true,
            val: b'W' as i32,
        },
        LongOpt {
            name: "height",
            has_arg: true,
            val: b'H' as i32,
        },
        LongOpt {
            name: "font-width",
            has_arg: true,
            val: b'x' as i32,
        },
        LongOpt {
            name: "font-height",
            has_arg: true,
            val: b'y' as i32,
        },
        LongOpt {
            name: "format",
            has_arg: true,
            val: b'f' as i32,
        },
        LongOpt {
            name: "dither",
            has_arg: true,
            val: b'd' as i32,
        },
        LongOpt {
            name: "gamma",
            has_arg: true,
            val: b'g' as i32,
        },
        LongOpt {
            name: "brightness",
            has_arg: true,
            val: b'b' as i32,
        },
        LongOpt {
            name: "contrast",
            has_arg: true,
            val: b'c' as i32,
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
        let c = g.next("W:H:f:d:g:b:c:hvx:y:", &long_options, None);
        if c == -1 {
            break;
        }
        let arg = g.optarg().unwrap_or("");
        match c {
            x if x == b'W' as i32 => cols = arg.parse().unwrap_or(0),
            x if x == b'H' as i32 => lines = arg.parse().unwrap_or(0),
            x if x == b'x' as i32 => font_width = arg.parse().unwrap_or(0),
            x if x == b'y' as i32 => font_height = arg.parse().unwrap_or(0),
            x if x == b'f' as i32 => format = arg.to_string(),
            x if x == b'd' as i32 => dither = Some(arg.to_string()),
            x if x == b'g' as i32 => gamma = arg.parse().unwrap_or(0.0),
            x if x == b'b' as i32 => brightness = arg.parse().unwrap_or(0.0),
            x if x == b'c' as i32 => contrast = arg.parse().unwrap_or(0.0),
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

    if font_height == 0 || font_width == 0 {
        eprintln!("{prog}: invalid font size {font_width}x{font_height}");
        return 1;
    }

    let mut cv = match Canvas::new(0, 0) {
        Ok(cv) => cv,
        Err(_) => {
            eprintln!("{prog}: unable to initialise libcaca");
            return 1;
        }
    };

    let filename = argv[argv.len() - 1].clone();
    let mut im = match Image::load(&filename) {
        Ok(im) => im,
        Err(_) => {
            eprintln!("{prog}: unable to load {filename}");
            return 1;
        }
    };

    if im.width() == 0 || im.height() == 0 {
        eprintln!(
            "{prog}: image {filename} has invalid dimensions {}x{}",
            im.width(),
            im.height()
        );
        return 1;
    }

    // Assume a 6x10 font.
    let (iw, ih) = (im.width() as i64, im.height() as i64);
    let (fw, fh) = (font_width as i64, font_height as i64);
    if cols == 0 && lines == 0 {
        cols = 60;
        lines = (cols as i64 * ih * fw / iw / fh) as i32;
    } else if cols != 0 && lines == 0 {
        lines = (cols as i64 * ih * fw / iw / fh) as i32;
    } else if cols == 0 && lines != 0 {
        cols = (lines as i64 * iw * fh / ih / fw) as i32;
    }

    if cv.set_size(cols, lines).is_err() {
        return 1;
    }
    if cv
        .set_color_ansi(Color::Default, Color::Transparent)
        .is_err()
    {
        return 1;
    }
    cv.clear();
    let algo = dither.clone().unwrap_or_else(|| "fstein".to_string());
    if im.set_algorithm(&algo).is_err() {
        eprintln!("{prog}: Can't dither image with algorithm '{algo}'");
        return -1;
    }

    if brightness != -1.0 {
        let _ = im.set_brightness(brightness);
    }
    if contrast != -1.0 {
        let _ = im.set_contrast(contrast);
    }
    if gamma != -1.0 {
        let _ = im.set_gamma(gamma);
    }

    if im.dither_onto(&mut cv, 0, 0, cols, lines).is_err() {
        return 1;
    }

    match cv.export_to_memory(&format) {
        Err(_) => {
            eprintln!("{prog}: Can't export to format '{format}'");
        }
        Ok(export) => {
            let _ = std::io::stdout().write_all(&export);
        }
    }

    0
}

fn main() {
    std::process::exit(run());
}
