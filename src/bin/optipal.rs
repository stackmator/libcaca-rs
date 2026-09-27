//! Port of libcaca's `tools/optipal.c`: S-Lang optimised palette generator.
//!
//! Computes the 128 directly-addressable S-Lang colour pairs (`base_colors`),
//! the emulated pairs (`emulated_colors`, stored with a +128 offset into the
//! association table) and the filler pairs (`unused_colors`), then prints the
//! `slang_palette` and `slang_assoc` tables exactly like the C tool. Pure
//! computation, no I/O beyond stdout.

const BLACK: i32 = 0;
const BLUE: i32 = 1;
const GREEN: i32 = 2;
const CYAN: i32 = 3;
const RED: i32 = 4;
const MAGENTA: i32 = 5;
const BROWN: i32 = 6;
const LIGHTGRAY: i32 = 7;
const DARKGRAY: i32 = 8;
#[allow(dead_code)]
const LIGHTBLUE: i32 = 9;
#[allow(dead_code)]
const LIGHTGREEN: i32 = 10;
#[allow(dead_code)]
const LIGHTCYAN: i32 = 11;
#[allow(dead_code)]
const LIGHTRED: i32 = 12;
#[allow(dead_code)]
const LIGHTMAGENTA: i32 = 13;
#[allow(dead_code)]
const YELLOW: i32 = 14;
const WHITE: i32 = 15;

/// 6 colours in hue order (mirrors `hue_list` in the C tool).
const HUE_LIST: [i32; 6] = [RED, BROWN, GREEN, CYAN, BLUE, MAGENTA];

struct Tables {
    assoc: [i32; 256],
    palette: [i32; 256],
}

fn at(assoc: &[i32; 256], fg: i32, bg: i32) -> i32 {
    assoc[(fg + 16 * bg) as usize]
}

impl Tables {
    fn setpair(&mut self, fg: i32, bg: i32, n: usize) {
        self.assoc[(fg + 16 * bg) as usize] = n as i32;
        self.palette[n] = fg + 16 * bg;
    }
}

fn base_colors(t: &mut Tables) {
    let mut cur = 0usize;
    for i in 1..16 {
        t.setpair(i, BLACK, cur);
        cur += 1;
    }

    t.setpair(BLACK, DARKGRAY, cur);
    cur += 1;
    t.setpair(DARKGRAY, LIGHTGRAY, cur);
    cur += 1;
    t.setpair(LIGHTGRAY, DARKGRAY, cur);
    cur += 1;
    t.setpair(WHITE, LIGHTGRAY, cur);
    cur += 1;
    t.setpair(LIGHTGRAY, WHITE, cur);
    cur += 1;

    for i in 1..7 {
        t.setpair(WHITE, i + 8, cur);
        cur += 1;
        t.setpair(i + 8, WHITE, cur);
        cur += 1;
        t.setpair(i, i + 8, cur);
        cur += 1;
        t.setpair(i + 8, i, cur);
        cur += 1;
        t.setpair(LIGHTGRAY, i + 8, cur);
        cur += 1;
        t.setpair(i + 8, LIGHTGRAY, cur);
        cur += 1;
        t.setpair(DARKGRAY, i, cur);
        cur += 1;
        t.setpair(i, DARKGRAY, cur);
        cur += 1;
        t.setpair(BLACK, i, cur);
        cur += 1;
    }

    for i in 0..6 {
        t.setpair(HUE_LIST[i], HUE_LIST[(i + 1) % 6], cur);
        cur += 1;
        t.setpair(HUE_LIST[(i + 1) % 6], HUE_LIST[i], cur);
        cur += 1;
        t.setpair(HUE_LIST[i] + 8, HUE_LIST[(i + 1) % 6] + 8, cur);
        cur += 1;
        t.setpair(HUE_LIST[(i + 1) % 6] + 8, HUE_LIST[i] + 8, cur);
        cur += 1;
    }

    for i in 0..6 {
        t.setpair(HUE_LIST[i], HUE_LIST[(i + 1) % 6] + 8, cur);
        cur += 1;
        t.setpair(HUE_LIST[(i + 1) % 6], HUE_LIST[i] + 8, cur);
        cur += 1;
        t.setpair(HUE_LIST[i] + 8, HUE_LIST[(i + 1) % 6], cur);
        cur += 1;
        t.setpair(HUE_LIST[(i + 1) % 6] + 8, HUE_LIST[i], cur);
        cur += 1;
    }

    t.setpair(BLACK, LIGHTGRAY, cur);
    cur += 1;
    t.setpair(BLACK, WHITE, cur);
    cur += 1;
    t.setpair(WHITE, DARKGRAY, cur);
    cur += 1;
    t.setpair(DARKGRAY, WHITE, cur);
    cur += 1;
    t.setpair(WHITE, BLUE, cur);
    cur += 1;
    t.setpair(LIGHTGRAY, BLUE, cur);
    cur += 1;

    debug_assert_eq!(cur, 128);
}

fn emulated_colors(t: &mut Tables) {
    for i in 1..7 {
        if i != BLUE {
            let n = 128 + t.assoc[(i + 8 + 16 * i) as usize];
            t.setpair(LIGHTGRAY, i, n as usize);
            let n = 128 + t.assoc[(LIGHTGRAY + 16 * (i + 8)) as usize];
            t.setpair(WHITE, i, n as usize);
        }
        let n = 128 + t.assoc[(DARKGRAY + 16 * i) as usize];
        t.setpair(BLACK, i + 8, n as usize);
        let n = 128 + t.assoc[(i + 16 * (i + 8)) as usize];
        t.setpair(DARKGRAY, i + 8, n as usize);
        let n = 128 + t.assoc[(i + 16 * DARKGRAY) as usize];
        t.setpair(i + 8, DARKGRAY, n as usize);
        let n = 128 + t.assoc[(i + 8 + 16 * LIGHTGRAY) as usize];
        t.setpair(i, LIGHTGRAY, n as usize);
        let n = 128 + t.assoc[(i + 8 + 16 * WHITE) as usize];
        t.setpair(i, WHITE, n as usize);
    }

    for i in 0..6 {
        // NB: a trailing `+ 8` / `+ 128` / `+ 136` inside the C index
        // shifts the fg / bg / both by 8 (dark <-> light), e.g.
        // `assoc[fg + 16*bg + 136]` reads `assoc[(fg+8) + 16*(bg+8)]`.
        let h1 = HUE_LIST[(i + 1) % 6];
        let h2 = HUE_LIST[(i + 2) % 6];
        let h4 = HUE_LIST[(i + 4) % 6];
        let h5 = HUE_LIST[(i + 5) % 6];
        let cur = HUE_LIST[i];
        t.setpair(h2, cur, (128 + at(&t.assoc, h1, cur)) as usize);
        t.setpair(
            h2 + 8,
            cur + 8,
            (128 + at(&t.assoc, h1 + 8, cur + 8)) as usize,
        );
        t.setpair(h2 + 8, cur, (128 + at(&t.assoc, h1 + 8, cur)) as usize);
        t.setpair(h2, cur + 8, (128 + at(&t.assoc, h1, cur + 8)) as usize);

        t.setpair(h4, cur, (128 + at(&t.assoc, h5, cur)) as usize);
        t.setpair(
            h4 + 8,
            cur + 8,
            (128 + at(&t.assoc, h5 + 8, cur + 8)) as usize,
        );
        t.setpair(h4 + 8, cur, (128 + at(&t.assoc, h5 + 8, cur)) as usize);
        t.setpair(h4, cur + 8, (128 + at(&t.assoc, h5, cur + 8)) as usize);
    }

    for i in 0..6 {
        t.setpair(
            HUE_LIST[i],
            HUE_LIST[(i + 3) % 6],
            (128 + at(&t.assoc, HUE_LIST[i], BLACK)) as usize,
        );
        t.setpair(
            HUE_LIST[i] + 8,
            HUE_LIST[(i + 3) % 6],
            (128 + at(&t.assoc, HUE_LIST[i] + 8, BLACK)) as usize,
        );
        t.setpair(
            HUE_LIST[(i + 3) % 6],
            HUE_LIST[i] + 8,
            (128 + at(&t.assoc, BLACK, HUE_LIST[i])) as usize,
        );
        t.setpair(
            HUE_LIST[(i + 3) % 6] + 8,
            HUE_LIST[i] + 8,
            (128 + at(&t.assoc, WHITE, HUE_LIST[i] + 8)) as usize,
        );
    }
}

fn unused_colors(t: &mut Tables) {
    let mut j = 0i32;
    for i in 0..256 {
        if t.palette[i] == -1 {
            t.setpair(j, j, i);
            j += 1;
        }
    }
}

fn main() {
    let mut t = Tables {
        assoc: [-1; 256],
        palette: [-1; 256],
    };

    base_colors(&mut t);
    emulated_colors(&mut t);
    unused_colors(&mut t);

    println!("static int const slang_palette[2*16*16] =");
    println!("{{");
    for (i, p) in t.palette.iter().enumerate() {
        if i % 8 == 0 {
            print!("    ");
        }
        print!("{:2}, {:2},  ", p % 16, p / 16);
        if i % 8 == 7 {
            println!();
        }
    }
    println!("}};");
    println!();

    println!("static int const slang_assoc[16*16] =");
    println!("{{");
    for (i, a) in t.assoc.iter().enumerate() {
        if i % 16 == 0 {
            print!("    ");
        }
        print!("{a}, ");
        if i % 16 == 15 {
            println!();
        }
    }
    println!("}};");
}
