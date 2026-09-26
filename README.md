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

## Display drivers

| Driver     | Description                                        |
| ---------- | -------------------------------------------------- |
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

## Status

The port covers the bulk of libcaca's public API (canvas, attributes, charset,
primitives, transforms, frames, codecs, dithering, bitmap fonts, FIGlet/TOIlet
fonts, display/events, the Win32 console driver, the DOS `conio` layer and option
parsing). Not yet ported: the graphical window drivers (X11/GL/cocoa/VGA) and
the zlib-backed `caca_file_*` compressed I/O, which are platform-specific or
need an external compression dependency.

Runnable programs live in `examples/`: `hello` (animated terminal demo),
`transform` (a port of libcaca's sprite/transform demo), `event` (a port of
the event lister; type "quit" to exit), `export` (a port of the codec demo
that renders the classic showcase canvas and exports it, e.g.
`export ansi`, `export tga`), `trifiller` (a port of the rotating textured
square demo; arrows move, `a`/`s` rotate, `q` quits), `font2tga` (renders every
glyph of the built-in font to a TGA image) and `font` (renders text to ARGB
and dithers it back for display). Set `CACA_DRIVER=null` to run them
headlessly.

## License

Distributed under the WTFPL v2, the same license as libcaca. The original C
library is © 2002–2021 Sam Hocevar and contributors.
