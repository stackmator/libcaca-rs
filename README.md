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

## Display drivers

| Driver     | Description                                    |
| ---------- | ---------------------------------------------- |
| `terminal` | ANSI/VT terminal with raw-mode input and resize |
| `raw`      | writes the native `caca` binary stream to stdout |
| `null`     | no output (headless rendering / tests)          |

The `terminal` driver uses the alternate screen, true colour when available,
and decodes ANSI/SGR input. On Unix it uses `termios`; on Windows it enables
virtual-terminal processing through the Win32 console API.

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
fonts, display/events and the DOS `conio` layer). Not yet ported: the
plugin-based windowing drivers (X11/GL/cocoa/win32/VGA) and the `file`/`getopt`
helpers that are unnecessary in idiomatic Rust.

See `examples/` for runnable programs.

## License

Distributed under the WTFPL v2, the same license as libcaca. The original C
library is © 2002–2021 Sam Hocevar and contributors.
