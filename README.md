# libcaca-rs

> **⚠️ Work in progress — this port is NOT finished.** It covers a large part of
> libcaca, but several subsystems are missing or only partially implemented (see
> [Status](#status)). It is not yet feature-complete nor a drop-in replacement
> for the original library. Expect breaking changes.

A **pure-Rust port of [libcaca]**, the Colour ASCII-Art library. This is *not* a
binding or wrapper: no C code is linked, and the public API is idiomatic safe
Rust. The rendering behaviour, attribute layout, character conversions and
drawing algorithms are ported from the original C sources.

[libcaca]: https://github.com/cacalabs/libcaca

```rust
use libcaca::{Canvas, Color};

let mut cv = Canvas::new(20, 5)?;
cv.set_color_ansi(Color::Yellow, Color::Blue)?;
cv.clear();
cv.put_str(2, 2, "Hello, caca!");
assert_eq!(cv.get_char(2, 2), b'H' as u32);
```

## Features

- **Canvas** — multi-frame character grid with 32-bit ARGB attributes, resizing,
  cropping (`set_boundaries`), blitting (with optional mask), cursor and handle,
  dirty-rectangle tracking.
- **Attributes & colours** — the exact libcaca attribute bit layout, ANSI/ARGB
  construction, and conversions to ANSI, RGB-12, RGB-24 and ARGB-64.
- **Charsets** — UTF-8/UTF-32, CP437 and ASCII conversions, fullwidth detection.
- **Drawing primitives** — solid and thin lines, polylines, boxes (ASCII, CP437),
  filled boxes, ellipses and circles, triangles (outline, filled, textured).
- **Transforms** — `invert`, `flip`, `flop`, `rotate_180`, `rotate_left/right`,
  `stretch_left/right`, with the original character-mirroring lookup tables.
- **Frames** — create/free/select/rename animation frames.
- **Import/export codecs** — native `caca`, `text`, `ansi`, `utf8`, `bin`; export
  to `html`, `html3`, `html5`, `bbfr`, `irc`, `ps`, `svg`, `tga`, `troff`.
- **Dithering** — 8/16/24/32-bit pixel sources, palette and mask formats,
  ordered/random/Floyd–Steinberg kernels, mono/gray/8/16/full colour modes.
- **Fonts** — load libcaca-format bitmap fonts (two built-in fonts are embedded)
  and rasterise a canvas to a 32-bit ARGB image buffer.
- **FIGfonts** — load FIGlet/TOIlet `.flf`/`.tlf` fonts with all horizontal
  smushing rules, kern/smush/overlap modes, wrapping and hardblank handling.
- **Display & events** — display contexts with `null`, `raw` (native binary to
  stdout) and `terminal` (ANSI/VT) drivers; raw-mode input and an event parser
  for keys, mouse (SGR) and resize.
- **DOS `conio` layer** — an idiomatic [`Conio`](src/conio.rs) type covering the
  functional subset of `caca_conio_*` (screen control, cursor, `putch`/`cputs`/
  `cprintf`, `getch`/`getche`/`getpass`, `kbhit`/`ungetch`, colours and delays).
- **Option parsing** — an idiomatic [`Getopt`](src/getopt.rs) port of
  `caca_getopt`, handling short-option bundles and `--long[=arg]` options.
- **File I/O** — a [`File`](src/file.rs) port of `caca_file_*` (open, read,
  write, tell, gets, eof). The opt-in `compression` feature adds transparent
  gzip and first-file-ZIP decompression via pure-Rust `flate2`, mirroring
  `HAVE_ZLIB_H`; importers and the FIGfont loader use it automatically.
- **Pre-1.0 compatibility** — an opt-in [`Compat`](src/compat.rs) type
  (`compat` cargo feature, off by default) covering the deprecated `caca0`
  API: old event encoding, features, sprites, bitmaps, colours and the full
  old drawing vocabulary, so legacy programs port without rewriting.
- **Image import** — an opt-in [`Image`](src/import.rs) type (`import`
  cargo feature, off by default) decoding image files through the pure-Rust
  `image` crate and dithering them with the `common-image.c` 32-bit RGBA
  layout, so `trifiller` can texture from a file like the C version.
- **VGA/DOS emulation** — opt-in `vga` and `dos` features porting the
  hardware drivers' logic as state (80x25 buffers, palette, cursor,
  press/release event synthesis). Physical screen/port/DOS access is
  impossible in user space; the portable logic is translated exactly and
  byte-tested, with no invented presentation.

## Display drivers

| Driver     | Description                                        |
| ---------- | -------------------------------------------------- |
| `winit`    | graphical window (`gui` feature)                    |
| `gl`       | OpenGL window (`gl` feature)                        |
| `cocoa`    | macOS window (`cocoa` feature, main thread only)    |
| `x11`      | X11 window (`x11` feature, Unix only)               |
| `win32`    | native Win32 console (Unicode, mouse) — Windows only |
| `terminal` | ANSI/VT terminal with raw-mode input and resize     |
| `raw`      | writes the native `caca` binary stream to stdout     |
| `null`     | no output (headless rendering / tests)              |

The `terminal` driver uses the alternate screen, true colour when available,
and decodes ANSI/SGR input. On Unix it uses `termios`; on Windows it enables
virtual-terminal processing through the Win32 console API. The `win32` driver
is a faithful port of libcaca's console driver: it writes a `CHAR_INFO` screen
buffer with `WriteConsoleOutputW` and reads `INPUT_RECORD`s (`ReadConsoleInput`),
with no ANSI sequences involved.

The `winit` driver (opt-in `gui` feature) is the pure-Rust answer to libcaca's
X11/GL window drivers: `winit` opens the window and feeds keyboard, mouse and
resize events, `softbuffer` presents the canvas rasterised with the built-in
bitmap font — no C libraries involved. It is never autodetected; request it
explicitly (`Display::with_driver(cv, Some("winit"))`) or set
`CACA_DRIVER=winit`. See `examples/gui.rs`.

The `gl` driver (opt-in `gl` feature) is the pure-Rust answer to libcaca's
GLUT driver: a `winit` window hosts an OpenGL context (via `glutin`), and the
canvas is uploaded as a texture and drawn as a fullscreen quad (via `glow`).
Request it explicitly with `Display::with_driver(cv, Some("gl"))`.

The `cocoa` driver (opt-in `cocoa` feature, macOS only) is the pure-Rust
answer to libcaca's Cocoa driver: an `NSWindow` shows an `NSImageView` fed
with the canvas rasterised by the built-in font, and input comes from the
`NSEvent` queue. AppKit requires the main thread, so constructing it
elsewhere fails cleanly; everything runs synchronously without surrendering
the run loop. Request it with `Display::with_driver(cv, Some("cocoa"))`.

All three graphical drivers are compile-verified; they need a display
server, GPU or macOS hardware at runtime to verify visually.

The `x11` driver (opt-in `x11` feature, Unix only) is a native port of
libcaca's X11 driver over pure-Rust `x11rb`: server-side core fonts,
vector box-drawing, dirty-rectangle rendering and full event handling
(keys via a US-layout table plus keycodes, mouse motion/buttons/wheel,
XFixes cursor hiding, resize, close). Like the other drivers it is never
autodetected — request `x11` explicitly.

```rust,no_run
use libcaca::{Canvas, Color, Display, EventMask};

let mut dp = Display::new(Canvas::new(80, 24)?)?;
loop {
    {
        let cv = dp.canvas_mut();
        cv.set_color_ansi(Color::White, Color::Blue)?;
        cv.clear();
        cv.put_str(1, 1, "Press any key to quit");
    }
    dp.refresh()?;
    if dp.get_event(EventMask::KEY_PRESS | EventMask::QUIT, 0).is_some() {
        break;
    }
}
```

## `no_std` support

Without the default `std` feature the crate is `no_std` + `alloc`: the canvas,
codec, dither, font, event-parsing and ANSI-rendering core keeps working, which
suits embedded or `wasm` targets. The OS-interactive layer needs `std` and is
gated behind the feature: display drivers (`Display`, `Driver`), the `Conio`
console, file-based helpers (`import_from_file`, `FigFont::load`),
wall-clock framerate timing and the opt-in `gui` window driver. `rand` stays
available but is deterministically
seeded without a clock. The pure `text` and `font2tga` examples build without
`std`; the rest require it.

```sh
cargo build --no-default-features
cargo test --no-default-features
```

## Status

The port covers the bulk of libcaca's public API (canvas, attributes, charset,
primitives, transforms, frames, codecs, dithering, bitmap fonts, FIGlet/TOIlet
fonts, display/events, the Win32 console driver, `winit`, X11 and OpenGL
graphical window drivers, the DOS `conio` layer, option parsing, file I/O with
transparent decompression, the `compat` pre-1.0 shim, the VGA/DOS
hardware logic (as emulation state) and image-file import via the `image`
crate. Not yet ported: the S-Lang/ncurses *bindings* (the `terminal` driver
speaks the same ANSI protocol without C).

Runnable programs live in `examples/`: `hello` (animated terminal demo),
`transform` (a port of libcaca's sprite/transform demo), `event` (a port of
the event lister; type "quit" to exit), `export` (a port of the codec demo
that renders the classic showcase canvas and exports it, e.g.
`export ansi`, `export tga`), `trifiller` (a port of the rotating textured
square demo; arrows move, `a`/`s` rotate, `q` quits), `font2tga` (renders every
glyph of the built-in font to a TGA image), `font` (renders text to ARGB
and dithers it back for display), `colors` (the 16x16 colour-pair chart),
`truecolor` (an ARGB gradient), `text` (mirrored ASCII-art import/export),
`blit` (a handle-centred sprite), `frames` (a 200-frame animation),
`fullwidth` (fullwidth glyph handling), `hsv` (a dithered HSV gradient) and
`driver` (the driver list with live switching), `gamma` (gamma-corrected
dithering under a moving mask), `mouse` (mouse tracking and buttons),
`figfont` (`figfont <font.flf> <text>`, rendering FIGlet fonts to UTF-8),
`unicode` (Unicode text, gradient blocks and double-width glyphs), `demo`
(the flagship animated menu: dots, lines, boxes, triangles, ellipses and a
dithered render), `import` (file viewer), `spritedit` (multi-frame sprite
round-trip), `input` (Unicode text-entry editor), `swallow` (a multiplexer
tiling four child `caca` streams), `snake` (the conio snake game),
`conio-snake` (its C++ duplicate, written against `Conio`), `connect4`
(the conio Connect-4 AI), `dithering` (the fuzzy-Voronoi dither test),
`demo0` (the 0.9-era `demo` twin on the old API, needs `--features compat`)
and `gui`
(the graphical window demo, needs `--features gui`). `trifiller` dithers an
image file into its texture when built with `--features import`.
Unit tests live alongside the modules; `tests/canvas_api.rs` ports the C API
stress test. Set `CACA_DRIVER=null` to run the examples headlessly.

## License

Distributed under the WTFPL v2, the same license as libcaca. The original C
library is © 2002–2021 Sam Hocevar and contributors.
