//! Port of libcaca's `examples/unicode.c`.
//!
//! Shows Unicode text, gradient blocks and double-width characters.
//! Press any key to quit when running interactively.

use libcaca::{Canvas, Color, Display, Driver, EventMask};

fn main() -> libcaca::Result<()> {
    let mut dp = Display::new(Canvas::new(0, 0)?)?;

    {
        let cv = dp.canvas_mut();
        cv.set_color_ansi(Color::White, Color::Blue)?;
        cv.put_str(1, 1, "Basic Unicode support");

        cv.set_color_ansi(Color::Default, Color::Transparent)?;
        cv.put_str(1, 2, "This is ASCII:    | abc DEF 123 !@# |");
        cv.put_str(1, 3, "This is Unicode:  | äßç δεφ ☺♥♀ ╞╬╗ |");
        cv.put_str(1, 4, "And this is, too: | ἀβϛ ΔЗҒ ᚴᛒᛯ ♩♔✈ |");
        cv.put_str(41, 4, "Size test: 018adxmygWX'_ÍçÕĔŷ ﻙ が本");

        cv.put_str(
            1,
            5,
            "If the three lines do not have the same length, there is a bug somewhere.",
        );

        cv.set_color_ansi(Color::White, Color::Blue)?;
        cv.put_str(1, 7, "Gradient glyphs");

        cv.set_color_ansi(Color::Default, Color::Transparent)?;
        cv.put_str(31, 8, "  0%");
        cv.put_str(31, 9, " 25%");
        cv.put_str(31, 10, " 50%");
        cv.put_str(31, 11, " 75%");
        cv.put_str(31, 12, "100%");

        cv.set_color_ansi(Color::LightRed, Color::LightGreen)?;
        cv.put_str(1, 8, "                             ");
        cv.put_str(1, 9, "░░░░░░░░░░░░░░░░░░░░░░░░░░░░░");
        cv.put_str(1, 10, "▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒");
        cv.put_str(1, 11, "▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓");
        cv.put_str(1, 12, "█████████████████████████████");

        cv.set_color_ansi(Color::LightGreen, Color::LightRed)?;
        cv.put_str(36, 8, "█████████████████████████████");
        cv.put_str(36, 9, "▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓");
        cv.put_str(36, 10, "▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒");
        cv.put_str(36, 11, "░░░░░░░░░░░░░░░░░░░░░░░░░░░░░");
        cv.put_str(36, 12, "                             ");

        cv.set_color_ansi(Color::White, Color::Blue)?;
        cv.put_str(1, 14, "Double width characters");

        cv.set_color_ansi(Color::LightRed, Color::Transparent)?;
        cv.put_str(1, 15, "| ドラゴン ボーレ |");
        cv.set_color_ansi(Color::Default, Color::Transparent)?;
        cv.put_str(1, 16, "| ()()()() ()()() |");
        cv.set_color_ansi(Color::Yellow, Color::Transparent)?;
        cv.put_str(1, 17, "| ドラゴン");
        cv.put_str(12, 17, "ボーレ |");

        cv.set_color_ansi(Color::Default, Color::Transparent)?;
        cv.put_str(
            1,
            18,
            "If the three lines do not have the same length, there is a bug somewhere.",
        );

        cv.put_str(
            1,
            20,
            "CP437 glyphs: ☺ ☻ ♥ ♦ ♣ ♠ • ◘ ○ ◙ ♂ ♀ ♪ ♫ ☼ ► ◄ ↕ ‼ ¶ § ▬ ↨ ↑ ↓ → ← ∟ ↔ ▲ ▼",
        );
        cv.put_str(
            1,
            21,
            "more CP437: α ß Γ π Σ σ µ τ Φ Θ Ω δ ∞ φ ε ∩ ≡ ± ≥ ≤ ⌠ ⌡ ÷ ≈ ° ∙ · √ ⁿ ² ■",
        );
        cv.put_str(
            1,
            22,
            "drawing blocks: ███ ▓▓▓ ▒▒▒ ░░░ ▀ ▄ ▌ ▐ █ ▖ ▗ ▘ ▝ ▚ ▞ ▙ ▛ ▜ ▟ ─ │ ┌ ┐ └ ┘ ├ ┤",
        );
        cv.put_str(
            1,
            23,
            "more drawing: ┬ ┴ ┼ ═ ║ ╒ ╓ ╔ ╕ ╖ ╗ ╘ ╙ ╚ ╛ ╜ ╝ ╞ ╟ ╠ ╡ ╢ ╣ ╤ ╥ ╦ ╧ ╨ ╩ ╪ ╫ ╬",
        );
        cv.put_str(1, 24, "misc Unicode: ● ☭ ☮ ☯ ♔ ♛ ♙ ♞ ⚒ ⚓ ⚠");
    }

    dp.refresh()?;

    if matches!(dp.driver(), Driver::Terminal | Driver::Win32) {
        loop {
            if dp
                .get_event(EventMask::KEY_PRESS | EventMask::QUIT, -1)
                .is_some()
            {
                break;
            }
        }
    }

    Ok(())
}
