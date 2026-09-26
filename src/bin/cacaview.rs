//! Port of libcaca's `src/cacaview.c`: image viewer for libcaca.
//!
//! Browse image files in the terminal with zoom (`+`/`-`/`x`), gamma
//! (`g`/`G`), pan (`hjkl`/arrows), dithering cycle (`d`/`D`), playlist
//! navigation (`n`/`p`, mouse buttons), help (`?`) and quit (`q`). The
//! `b`/`B`/`a`/`A` background/antialiasing keys are absent exactly like in
//! the C version, where they sit inside `#if 0`.
//!
//! Requires the `import` cargo feature.

use libcaca::{key, Canvas, Display, Dither, Event, EventMask, Image};

// The C `MODE_IMAGE`/`MODE_FILES` constants describe a `mode` variable that
// is never used, so they are omitted here. Likewise `STATUS_ANTIALIASING`
// and `STATUS_BACKGROUND` only appear inside the C `#if 0` blocks, so only
// `STATUS_DITHERING` survives.
const STATUS_DITHERING: u32 = 1;

const ZOOM_FACTOR: f32 = 1.08;
const ZOOM_MAX: usize = 50;
const GAMMA_FACTOR: f32 = 1.04;
const GAMMA_MAX: usize = 100;
const PAD_STEP: f32 = 0.15;

struct Viewer {
    dp: Display,
    im: Option<Image>,
    zoomtab: [f32; ZOOM_MAX + 1],
    gammatab: [f32; GAMMA_MAX + 1],
    xfactor: f32,
    yfactor: f32,
    dx: f32,
    dy: f32,
    zoom: i32,
    gamma: i32,
    fullscreen: bool,
    ww: i32,
    wh: i32,
    list: Vec<String>,
    current: usize,
    quit: bool,
    update: bool,
    help: bool,
    status: u32,
    reload: bool,
    dither_algorithm: usize,
}

impl Viewer {
    fn gamma_value(&self) -> f32 {
        if self.gamma < 0 {
            1.0 / self.gammatab[(-self.gamma) as usize]
        } else {
            self.gammatab[self.gamma as usize]
        }
    }

    fn set_zoom(&mut self, new_zoom: i32) {
        if self.im.is_none() {
            return;
        }
        let im = self.im.as_ref().unwrap();

        self.zoom = new_zoom.clamp(-(ZOOM_MAX as i32), ZOOM_MAX as i32);

        let ww = self.dp.canvas().width();
        let height = if self.fullscreen {
            self.wh
        } else {
            self.wh - 3
        };

        self.xfactor = if self.zoom < 0 {
            1.0 / self.zoomtab[(-self.zoom) as usize]
        } else {
            self.zoomtab[self.zoom as usize]
        };
        self.yfactor = self.xfactor * ww as f32 / height as f32 * im.height() as f32
            / im.width() as f32
            * self.dp.canvas().height() as f32
            / self.dp.canvas().width() as f32
            * self.dp.display_width() as f32
            / self.dp.display_height() as f32;

        if self.yfactor > self.xfactor {
            let tmp = self.xfactor;
            self.xfactor = tmp * tmp / self.yfactor;
            self.yfactor = tmp;
        }
    }

    fn set_gamma(&mut self, new_gamma: i32) {
        if self.im.is_none() {
            return;
        }

        self.gamma = new_gamma.clamp(-(GAMMA_MAX as i32), GAMMA_MAX as i32);

        let value = self.gamma_value();
        let _ = self.im.as_mut().unwrap().set_gamma(value);
    }

    fn draw_checkers(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let cw = self.dp.canvas().width();
        let ch = self.dp.canvas().height();
        let (w, h) = (
            if x + w > cw { cw - x } else { w },
            if y + h > ch { ch - y } else { h },
        );

        for yn in y.max(0)..y + h {
            for xn in x.max(0)..x + w {
                let cv = self.dp.canvas_mut();
                if (((xn - x) / 5) ^ ((yn - y) / 3)) & 1 != 0 {
                    let _ = cv.set_color_ansi(libcaca::Color::LightGray, libcaca::Color::DarkGray);
                } else {
                    let _ = cv.set_color_ansi(libcaca::Color::DarkGray, libcaca::Color::LightGray);
                }
                cv.put_char(xn, yn, b' ' as u32);
            }
        }
    }

    fn print_status(&mut self) {
        use libcaca::Color::{Black, Blue, LightGray, White};
        let (ww, wh) = (self.ww, self.wh);
        let gamma = self.gamma_value();
        let sign = if self.zoom > 0 { "+" } else { "" };
        let zoom = self.zoom;
        let cv = self.dp.canvas_mut();
        let _ = cv.set_color_ansi(White, Blue);
        cv.draw_line(0, 0, ww - 1, 0, b' ' as u32);
        cv.draw_line(0, wh - 2, ww - 1, wh - 2, b'-' as u32);
        cv.put_str(
            0,
            0,
            "q:Quit  np:Next/Prev  +-x:Zoom  gG:Gamma  \
             hjkl:Move  d:Dither  a:Antialias",
        );
        cv.put_str(ww - 6, 0, "?:Help");
        cv.printf(
            3,
            wh - 2,
            format_args!("cacaview {}", env!("CARGO_PKG_VERSION")),
        );
        // C uses `%#.3g` (3 significant digits); 3 decimals is the
        // straight-line equivalent.
        cv.printf(ww - 30, wh - 2, format_args!("(gamma: {gamma:.3})"));
        cv.printf(ww - 14, wh - 2, format_args!("(zoom: {sign}{zoom})"));

        let _ = cv.set_color_ansi(LightGray, Black);
        cv.draw_line(0, wh - 1, ww - 1, wh - 1, b' ' as u32);
    }

    fn print_help(&mut self, x: i32, y: i32) {
        static HELP: &[&str] = &[
            " +: zoom in              ",
            " -: zoom out             ",
            " g: decrease gamma       ",
            " G: increase gamma       ",
            " x: reset zoom and gamma ",
            " ----------------------- ",
            " hjkl: move view         ",
            " arrows: move view       ",
            " ----------------------- ",
            " a: antialiasing method  ",
            " d: dithering method     ",
            " b: background mode      ",
            " ----------------------- ",
            " ?: help                 ",
            " q: quit                 ",
        ];

        use libcaca::Color::{Blue, White};
        let cv = self.dp.canvas_mut();
        let _ = cv.set_color_ansi(White, Blue);
        for (i, line) in HELP.iter().enumerate() {
            cv.put_str(x, y + i as i32, line);
        }
    }

    fn next_file(&mut self) {
        if !self.list.is_empty() {
            self.current = (self.current + 1) % self.list.len();
        }
        self.reload = true;
    }

    fn prev_file(&mut self) {
        if !self.list.is_empty() {
            self.current = (self.current + self.list.len() - 1) % self.list.len();
        }
        self.reload = true;
    }

    fn handle_key(&mut self, ch: i32) {
        let mut new_status = 0;
        match ch {
            x if x == b'n' as i32 || x == b'N' as i32 => self.next_file(),
            x if x == b'p' as i32 || x == b'P' as i32 => self.prev_file(),
            x if x == b'f' as i32 || x == b'F' as i32 || x == key::F11 => {
                self.fullscreen = !self.fullscreen;
                self.update = true;
                self.set_zoom(self.zoom);
            }
            // `b`/`B`/`a`/`A` are inside `#if 0` in the C version.
            x if x == b'd' as i32 => {
                let count = Dither::algorithm_list().len() / 2;
                self.dither_algorithm = (self.dither_algorithm + 1) % count;
                if let Some(im) = self.im.as_mut() {
                    let _ = im.set_algorithm(Dither::algorithm_list()[self.dither_algorithm * 2]);
                }
                new_status = STATUS_DITHERING;
                self.update = true;
            }
            x if x == b'D' as i32 => {
                let count = Dither::algorithm_list().len() / 2;
                self.dither_algorithm = (self.dither_algorithm + count - 1) % count;
                if let Some(im) = self.im.as_mut() {
                    let _ = im.set_algorithm(Dither::algorithm_list()[self.dither_algorithm * 2]);
                }
                new_status = STATUS_DITHERING;
                self.update = true;
            }
            x if x == b'+' as i32 => {
                self.update = true;
                self.set_zoom(self.zoom + 1);
            }
            x if x == b'-' as i32 => {
                self.update = true;
                self.set_zoom(self.zoom - 1);
            }
            x if x == b'G' as i32 => {
                self.update = true;
                self.set_gamma(self.gamma + 1);
            }
            x if x == b'g' as i32 => {
                self.update = true;
                self.set_gamma(self.gamma - 1);
            }
            x if x == b'x' as i32 || x == b'X' as i32 => {
                self.update = true;
                self.set_zoom(0);
                self.set_gamma(0);
            }
            x if x == b'k' as i32 || x == b'K' as i32 || x == key::UP => {
                if self.yfactor > 1.0 {
                    self.dy -= PAD_STEP / self.yfactor;
                }
                if self.dy < 0.0 {
                    self.dy = 0.0;
                }
                self.update = true;
            }
            x if x == b'j' as i32 || x == b'J' as i32 || x == key::DOWN => {
                if self.yfactor > 1.0 {
                    self.dy += PAD_STEP / self.yfactor;
                }
                if self.dy > 1.0 {
                    self.dy = 1.0;
                }
                self.update = true;
            }
            x if x == b'h' as i32 || x == b'H' as i32 || x == key::LEFT => {
                if self.xfactor > 1.0 {
                    self.dx -= PAD_STEP / self.xfactor;
                }
                if self.dx < 0.0 {
                    self.dx = 0.0;
                }
                self.update = true;
            }
            x if x == b'l' as i32 || x == b'L' as i32 || x == key::RIGHT => {
                if self.xfactor > 1.0 {
                    self.dx += PAD_STEP / self.xfactor;
                }
                if self.dx > 1.0 {
                    self.dx = 1.0;
                }
                self.update = true;
            }
            x if x == b'?' as i32 => {
                self.help = !self.help;
                self.update = true;
            }
            x if x == b'q' as i32 || x == b'Q' as i32 || x == key::ESCAPE => {
                self.quit = true;
            }
            _ => {}
        }
        if new_status != 0 {
            self.status = new_status;
        }
    }

    fn reload_current(&mut self) {
        use libcaca::Color::{Blue, White};
        let file = self.list[self.current].clone();
        let (ww, wh) = (self.ww, self.wh);

        // Truncate the message to the screen width, like `buffer[ww] = 0`.
        let mut msg = format!(" Loading `{file}'... ").into_bytes();
        msg.truncate(ww.max(0) as usize);
        let msg = String::from_utf8_lossy(&msg).into_owned();
        let x = (ww - msg.len() as i32) / 2;

        {
            let cv = self.dp.canvas_mut();
            let _ = cv.set_color_ansi(White, Blue);
            cv.put_str(x, wh / 2, &msg);
        }
        let _ = self.dp.refresh();
        self.ww = self.dp.canvas().width();
        self.wh = self.dp.canvas().height();

        self.im = None;
        self.im = Image::load(&file).ok();
        self.reload = false;

        // Reset image-specific runtime variables.
        self.dx = 0.5;
        self.dy = 0.5;
        self.update = true;
        self.set_zoom(0);
        self.set_gamma(0);
    }

    fn draw(&mut self) {
        use libcaca::Color::{Black, Blue, LightGray, White};
        let (ww, wh) = (self.ww, self.wh);
        {
            let cv = self.dp.canvas_mut();
            let _ = cv.set_color_ansi(White, Black);
            cv.clear();
        }

        if self.list.is_empty() {
            let cv = self.dp.canvas_mut();
            let _ = cv.set_color_ansi(White, Blue);
            cv.printf(ww / 2 - 5, wh / 2, format_args!(" No image. "));
        } else if self.im.is_none() {
            let file = self.list[self.current].clone();
            let mut msg = format!(" Error loading `{file}'. ").into_bytes();
            msg.truncate(ww.max(0) as usize);
            let msg = String::from_utf8_lossy(&msg).into_owned();
            let x = (ww - msg.len() as i32) / 2;
            let cv = self.dp.canvas_mut();
            let _ = cv.set_color_ansi(White, Blue);
            cv.put_str(x, wh / 2, &msg);
        } else {
            let y = if self.fullscreen { 0 } else { 1 };
            let height = if self.fullscreen { wh } else { wh - 3 };

            let xdelta = if self.xfactor > 1.0 { self.dx } else { 0.5 };
            let ydelta = if self.yfactor > 1.0 { self.dy } else { 0.5 };

            self.draw_checkers(
                (ww as f32 * (1.0 - self.xfactor) / 2.0) as i32,
                y + (height as f32 * (1.0 - self.yfactor) / 2.0) as i32,
                (ww as f32 * self.xfactor) as i32,
                (height as f32 * self.yfactor) as i32,
            );

            if let Some(im) = self.im.as_ref() {
                let cv = self.dp.canvas_mut();
                let _ = im.dither_onto(
                    cv,
                    (ww as f32 * (1.0 - self.xfactor) * xdelta) as i32,
                    y + (height as f32 * (1.0 - self.yfactor) * ydelta) as i32,
                    (ww as f32 * self.xfactor + 1.0) as i32,
                    (height as f32 * self.yfactor + 1.0) as i32,
                );
            }
        }

        if !self.fullscreen {
            self.print_status();

            if self.status == STATUS_DITHERING {
                let algo = self.im.as_ref().map(Image::algorithm).unwrap_or("");
                let cv = self.dp.canvas_mut();
                let _ = cv.set_color_ansi(LightGray, Black);
                cv.printf(0, wh - 1, format_args!("Dithering: {algo}"));
            }
        }

        if self.help {
            self.print_help(ww - 26, 2);
        }

        let _ = self.dp.refresh();
        self.update = false;
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv[0].clone();

    let cv = match Canvas::new(0, 0) {
        Ok(cv) => cv,
        Err(_) => {
            eprintln!("{prog}: unable to initialise libcaca");
            std::process::exit(1);
        }
    };
    let mut v = Viewer {
        dp: match Display::new(cv) {
            Ok(dp) => dp,
            Err(_) => {
                eprintln!("{prog}: unable to initialise libcaca");
                std::process::exit(1);
            }
        },
        im: None,
        zoomtab: [1.0; ZOOM_MAX + 1],
        gammatab: [1.0; GAMMA_MAX + 1],
        xfactor: 1.0,
        yfactor: 1.0,
        dx: 0.5,
        dy: 0.5,
        zoom: 0,
        gamma: 0,
        fullscreen: false,
        ww: 0,
        wh: 0,
        list: Vec::new(),
        current: 0,
        quit: false,
        update: true,
        help: false,
        status: 0,
        reload: false,
        dither_algorithm: 0,
    };

    let _ = v.dp.set_title("cacaview");

    v.ww = v.dp.canvas().width();
    v.wh = v.dp.canvas().height();

    // Fill the zoom table.
    for i in 0..ZOOM_MAX {
        v.zoomtab[i + 1] = v.zoomtab[i] * ZOOM_FACTOR;
    }

    // Fill the gamma table.
    for i in 0..GAMMA_MAX {
        v.gammatab[i + 1] = v.gammatab[i] * GAMMA_FACTOR;
    }

    // Load items into the playlist, skipping options except after `--`.
    let mut opts = true;
    for arg in argv.iter().skip(1) {
        if opts && arg.starts_with('-') {
            if arg == "--" {
                opts = false;
            }
            continue;
        }
        v.list.push(arg.clone());
        v.reload = true;
    }

    // Go!
    while !v.quit {
        let mask =
            EventMask::KEY_PRESS | EventMask::RESIZE | EventMask::MOUSE_PRESS | EventMask::QUIT;
        let mut event = if v.update {
            v.dp.get_event(mask, 0)
        } else {
            v.dp.get_event(mask, -1)
        };

        while let Some(ev) = event {
            match ev {
                Event::MousePress { button, .. } => {
                    if button == 1 {
                        v.next_file();
                    }
                    if button == 2 {
                        v.prev_file();
                    }
                }
                Event::KeyPress(k) => v.handle_key(k.ch),
                Event::Resize { w, h } => {
                    let _ = v.dp.refresh();
                    v.ww = w;
                    v.wh = h;
                    v.update = true;
                    v.set_zoom(v.zoom);
                }
                Event::Quit => v.quit = true,
                _ => {}
            }

            event = v.dp.get_event(EventMask::KEY_PRESS, 0);
        }

        if !v.list.is_empty() && v.reload {
            v.reload_current();
        }

        v.draw();
    }
}
