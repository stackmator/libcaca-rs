//! Port of libcaca's `examples/conio.c` (Connect-4 vs. the computer).
//!
//! A full 4-in-line game with AI on the DOS-style [`Conio`] console. Move with
//! the arrow keys, drop a piece with space, Escape quits. The original mixes
//! raw `printf` (which relies on terminal newlines) with conio calls; this
//! port routes everything through an `emit` helper that honours `\n`.
//!
//! Headless runs auto-play random moves for a bounded number of plies.

use libcaca::conio::Conio;
use libcaca::{key, rand};

const X_BOARD: usize = 8;
const Y_BOARD: usize = 8;

const WIN: i32 = 1;
const LOSE: i32 = -1;
const DRAW: i32 = 0;
const OK: i32 = 2;

const COMPUTER: i32 = 0;
const HUMAN: i32 = 1;
const EMPTY: i32 = -1;
const BASE: i32 = -2;

const WIN_MESSAGE: &str = "I Win! Press Any Key To Continue...\n";
const LOSE_MESSAGE: &str = "You Win! Press Any Key To Continue...\n";
const DRAW_MESSAGE: &str = "Draw! Press Any Key To Continue...\n";

type Board = [[i32; X_BOARD]; Y_BOARD];

struct Game {
    con: Conio,
    mv: [usize; X_BOARD],
    col: [i32; X_BOARD],
    square: Board,
    interactive: bool,
    plies: u32,
}

impl Game {
    /// Print text honouring `\n` by moving the cursor, starting at `(x, y)`.
    fn emit(&mut self, x: i32, y: i32, s: &str) {
        for (i, line) in s.split('\n').enumerate() {
            self.con.gotoxy(x, y + i as i32);
            self.con.cputs(line);
        }
    }

    fn computer_move(&mut self) -> bool {
        if self.is_full() {
            return false;
        }
        let x_best = self.bestmove();
        self.emit(
            52,
            2,
            &format!(
                "x:{}, y:{}\n",
                x_best + 1,
                Y_BOARD as i32 - y_base(x_best, &self.square)
            ),
        );
        let mut sq = self.square;
        make_move(COMPUTER, x_best, &mut sq);
        self.square = sq;
        true
    }

    fn double_head(who: i32, xsquare: &Board) -> bool {
        xsquare
            .iter()
            .any(|row| row.windows(5).any(|w| w == [BASE, who, who, who, BASE]))
    }

    fn human_move(&mut self, x: i32) -> bool {
        self.con.gotoxy(1, 20);
        if x < 0 || x as usize >= X_BOARD {
            return false;
        }
        if y_base(x, &self.square) == -1 {
            return false;
        }
        if self.is_full() {
            return false;
        }
        self.emit(
            52,
            5,
            &format!(
                "x:{}, y:{}\n",
                x + 1,
                Y_BOARD as i32 - y_base(x, &self.square)
            ),
        );
        let mut sq = self.square;
        make_move(HUMAN, x, &mut sq);
        self.square = sq;
        true
    }

    fn is_full(&self) -> bool {
        for x in 0..X_BOARD {
            if self.square[0][x] == EMPTY || self.square[0][x] == BASE {
                return false;
            }
        }
        true
    }

    fn is_won(who: i32, xsquare: &Board) -> bool {
        for x in 0..X_BOARD {
            for y in 0..Y_BOARD {
                if x + 3 < X_BOARD
                    && xsquare[y][x] == who
                    && xsquare[y][x + 1] == who
                    && xsquare[y][x + 2] == who
                    && xsquare[y][x + 3] == who
                {
                    return true;
                }
                if y + 3 < Y_BOARD
                    && xsquare[y][x] == who
                    && xsquare[y + 1][x] == who
                    && xsquare[y + 2][x] == who
                    && xsquare[y + 3][x] == who
                {
                    return true;
                }
                if x + 3 < X_BOARD
                    && y + 3 < Y_BOARD
                    && xsquare[y][x] == who
                    && xsquare[y + 1][x + 1] == who
                    && xsquare[y + 2][x + 2] == who
                    && xsquare[y + 3][x + 3] == who
                {
                    return true;
                }
                if x + 3 < X_BOARD
                    && y >= 3
                    && xsquare[y][x] == who
                    && xsquare[y - 1][x + 1] == who
                    && xsquare[y - 2][x + 2] == who
                    && xsquare[y - 3][x + 3] == who
                {
                    return true;
                }
            }
        }
        false
    }

    fn bestmove(&mut self) -> i32 {
        let mut xsquare;
        let mut n = [0i32; X_BOARD];

        self.con.gotoxy(1, 19);
        self.con.textcolor(4);
        if Self::x_won(COMPUTER, &self.square) != -1 {
            self.con
                .cprintf(format_args!("Computer Previous Depth : +1\n"));
            return Self::x_won(COMPUTER, &self.square);
        }
        if Self::x_won(HUMAN, &self.square) != -1 {
            self.con
                .cprintf(format_args!("Computer Previous Depth : -1\n"));
            return Self::x_won(HUMAN, &self.square);
        }
        for &m in &self.mv {
            if y_base(m as i32, &self.square) != -1 && self.col[m] == COMPUTER {
                xsquare = self.square;
                make_move(COMPUTER, m as i32, &mut xsquare);
                if Self::x_won(HUMAN, &xsquare) == -1 {
                    self.con
                        .cprintf(format_args!("Computer Previous Depth : +2\n"));
                    return m as i32;
                }
            }
        }
        if Self::x_double_head(COMPUTER, &self.square) != -1 {
            xsquare = self.square;
            make_move(
                COMPUTER,
                Self::x_double_head(COMPUTER, &xsquare),
                &mut xsquare,
            );
            if Self::x_won(HUMAN, &xsquare) == -1 {
                self.con
                    .cprintf(format_args!("Computer Previous Depth : +3\n"));
                return Self::x_double_head(COMPUTER, &self.square);
            }
        }

        if Self::x_double_head(HUMAN, &self.square) != -1 {
            xsquare = self.square;
            make_move(COMPUTER, Self::x_double_head(HUMAN, &xsquare), &mut xsquare);
            if Self::x_won(HUMAN, &xsquare) == -1 {
                self.con
                    .cprintf(format_args!("Computer Previous Depth : -3\n"));
                return Self::x_double_head(HUMAN, &self.square);
            }
        }

        let snake = self.x_two_snake(COMPUTER);
        if snake != -1 {
            xsquare = self.square;
            make_move(COMPUTER, snake, &mut xsquare);
            if Self::x_won(HUMAN, &xsquare) == -1 {
                self.con
                    .cprintf(format_args!("Computer Previous Depth : +4\n"));
                return snake;
            }
        }
        if self.x_two_snake(HUMAN) != -1 {
            xsquare = self.square;
            make_move(COMPUTER, self.x_two_snake(HUMAN), &mut xsquare);
            if Self::x_won(HUMAN, &xsquare) == -1 {
                self.con
                    .cprintf(format_args!("Computer Previous Depth : -4\n"));
                return self.x_two_snake(HUMAN);
            }
        }

        Self::gen_num_win(&self.square, &mut n);
        Self::sorting(&self.mv, &mut n);

        for &col in n.iter() {
            if y_base(col, &self.square) != -1 {
                xsquare = self.square;
                make_move(COMPUTER, col, &mut xsquare);
                if Self::x_won(HUMAN, &xsquare) == -1 {
                    self.con
                        .cprintf(format_args!("Computer Previous Depth : +5\n"));
                    return col;
                }
            }
        }

        for &m in &self.mv {
            if y_base(m as i32, &self.square) != -1 {
                self.con
                    .cprintf(format_args!("Computer Previous Depth : +0\n"));
                return m as i32;
            }
        }
        -1
    }

    fn status(&self) -> i32 {
        if Self::is_won(COMPUTER, &self.square) {
            WIN
        } else if Self::is_won(HUMAN, &self.square) {
            LOSE
        } else if self.is_full() {
            DRAW
        } else {
            OK
        }
    }

    fn x_double_head(who: i32, xsquare: &Board) -> i32 {
        for x in 0..X_BOARD {
            if y_base(x as i32, xsquare) != -1 {
                let mut xxsquare = *xsquare;
                make_move(who, x as i32, &mut xxsquare);
                if Self::double_head(who, &xxsquare) {
                    return x as i32;
                }
            }
        }
        -1
    }

    fn x_two_snake(&mut self, who: i32) -> i32 {
        // NOTE: takes &mut only for `two_snake`'s scratch use of nothing
        // shared; kept as a method for symmetry with the C version.
        let sq = self.square;
        let mv = self.mv;
        let col = self.col;
        for x in 0..X_BOARD {
            if y_base(mv[x] as i32, &sq) != -1 {
                let mut xxsquare = sq;
                make_move(who, mv[x] as i32, &mut xxsquare);
                for dx in 0..X_BOARD {
                    if Self::two_snake_static(who, mv[dx], &xxsquare) && col[mv[dx]] != who {
                        if who == COMPUTER {
                            self.col[mv[dx]] = who;
                        }
                        return mv[x] as i32;
                    }
                }
            }
        }
        -1
    }

    fn two_snake_static(who: i32, x: usize, xsquare: &Board) -> bool {
        let mut n = [false; Y_BOARD];
        for i in 0..Y_BOARD {
            if xsquare[i][x] == BASE || xsquare[i][x] == EMPTY {
                let mut xxsquare = *xsquare;
                xxsquare[i][x] = who;
                if Self::is_won(who, &xxsquare) {
                    n[i] = true;
                }
            }
        }
        for i in 0..(Y_BOARD - 1) {
            if n[i] && n[i + 1] {
                return true;
            }
        }
        false
    }

    fn x_won(who: i32, xsquare: &Board) -> i32 {
        for x in 0..X_BOARD {
            if y_base(x as i32, xsquare) != -1 {
                let mut xxsquare = *xsquare;
                make_move(who, x as i32, &mut xxsquare);
                if Self::is_won(who, &xxsquare) {
                    return x as i32;
                }
            }
        }
        -1
    }

    // Index loops mirror the C version's structure; iterators would fight
    // the borrow checker here because the body re-indexes `n`.
    #[allow(clippy::needless_range_loop)]
    fn gen_num_win(square: &Board, n: &mut [i32; X_BOARD]) {
        for i in 0..X_BOARD {
            n[i] = 0;
            if y_base(i as i32, square) != -1 {
                let mut xsquare = *square;
                make_move(COMPUTER, i as i32, &mut xsquare);
                for j in 0..X_BOARD {
                    for k in 0..Y_BOARD {
                        if xsquare[k][j] == EMPTY || xsquare[k][j] == BASE {
                            let mut xxsquare = xsquare;
                            xxsquare[k][j] = COMPUTER;
                            if Self::is_won(COMPUTER, &xxsquare) {
                                n[i] += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    fn draw_board(&mut self) {
        self.con.textcolor(0);
        self.con.textbackground(7);
        self.con.clrscr();
        self.emit(
            1,
            1,
            "+-----+-----+-----+-----+-----+-----+-----+-----+\n\
             |     |     |     |     |     |     |     |     |\n\
             +-----+-----+-----+-----+-----+-----+-----+-----+\n\
             |     |     |     |     |     |     |     |     |\n\
             +-----+-----+-----+-----+-----+-----+-----+-----+\n\
             |     |     |     |     |     |     |     |     |\n\
             +-----+-----+-----+-----+-----+-----+-----+-----+\n\
             |     |     |     |     |     |     |     |     |\n\
             +-----+-----+-----+-----+-----+-----+-----+-----+\n\
             |     |     |     |     |     |     |     |     |\n\
             +-----+-----+-----+-----+-----+-----+-----+-----+\n\
             |     |     |     |     |     |     |     |     |\n\
             +-----+-----+-----+-----+-----+-----+-----+-----+\n\
             |     |     |     |     |     |     |     |     |\n\
             +-----+-----+-----+-----+-----+-----+-----+-----+\n\
             |     |     |     |     |     |     |     |     |\n\
             +-----+-----+-----+-----+-----+-----+-----+-----+\n\
             ARROW:move\tSPACE:select\tESC:exit\n",
        );
        self.con.textcolor(1);
        self.emit(44, 20, "4 In Line\n");
        self.emit(44, 21, "ver Beta by Cheok Yan Cheng\n");
        self.emit(44, 22, "E-mail : yccheok@yahoo.com\n");
        self.emit(44, 23, "Web Site: www.geocities.com/yccheok\n");
        self.emit(44, 24, "Source code included!\n");
        self.emit(1, 22, "Human's Piece is O\n");
        self.emit(1, 23, "Computer's Piece is X\n");
        self.emit(52, 1, "Computer Move :\n");
        self.emit(52, 4, "Human Move :\n");
    }

    fn draw_piece(&mut self) {
        for x in 0..X_BOARD {
            for y in 0..Y_BOARD {
                if self.square[y][x] == HUMAN {
                    self.con.gotoxy(x as i32 * 6 + 4, y as i32 * 2 + 2);
                    self.con.textcolor(1);
                    self.con.cputs("O");
                } else if self.square[y][x] == COMPUTER {
                    self.con.gotoxy(x as i32 * 6 + 4, y as i32 * 2 + 2);
                    self.con.textcolor(4);
                    self.con.cputs("X");
                }
            }
        }
    }

    fn get_human_move(&mut self) {
        let mut x = 3i32;
        loop {
            self.con.gotoxy(x * 6 + 4, 2);
            let ch = self.con.getch();
            if ch == key::LEFT {
                if x > 0 {
                    x -= 1;
                }
            } else if ch == key::RIGHT {
                if x < X_BOARD as i32 - 1 {
                    x += 1;
                }
            } else if ch == key::ESCAPE {
                self.con.textcolor(7);
                self.con.textbackground(0);
                self.con.clrscr();
                self.emit(
                    1,
                    1,
                    "Thank You For Playing 4 in line by Cheok Yan Cheng!\n",
                );
                std::process::exit(0);
            } else if ch == b' ' as i32 {
                if self.human_move(x) {
                    self.draw_piece();
                    return;
                } else {
                    self.con.gotoxy(1, 20);
                    self.con.textcolor(4);
                    self.con.cputs("OOPs! Wrong Move! ");
                }
            }
            if !self.interactive {
                // Headless safety net: never spin on input.
                return;
            }
        }
    }

    fn init(&mut self) {
        for x in 0..X_BOARD {
            for y in 0..(Y_BOARD - 1) {
                self.square[y][x] = EMPTY;
            }
            self.square[Y_BOARD - 1][x] = BASE;
            self.col[x] = -1;
        }
    }

    fn sorting(mv: &[usize; X_BOARD], n: &mut [i32; X_BOARD]) {
        let mut store = [0i32; X_BOARD];
        for slot in store.iter_mut() {
            let alpha = *n.iter().max().unwrap_or(&0);
            for &m in mv.iter() {
                if n[m] == alpha {
                    *slot = m as i32;
                    n[m] = -1;
                    break;
                }
            }
        }
        n.copy_from_slice(&store);
    }

    fn play(&mut self) {
        let mut myturn = true;
        self.draw_board();
        // Warm up the generator like srand(time(NULL)); rand(); does.
        let _ = rand(0, 2);

        loop {
            match self.status() {
                WIN | LOSE | DRAW => {
                    self.init();
                    self.draw_board();
                    if myturn {
                        let mv = 2 + rand(0, 4);
                        let mut sq = self.square;
                        make_move(COMPUTER, mv, &mut sq);
                        self.square = sq;
                    }
                    myturn = !myturn;
                    self.draw_piece();
                }
                _ => {}
            }

            self.con.textcolor(4);
            self.con.gotoxy(1, 20);
            self.con.cputs("Your Turn, Please.");

            if self.interactive {
                self.get_human_move();
            } else {
                // Headless: drop on a random legal column.
                self.plies += 1;
                if self.plies > 60 {
                    return;
                }
                let mut placed = false;
                for _ in 0..32 {
                    let x = rand(0, X_BOARD as i32);
                    if self.human_move(x) {
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    return;
                }
                self.draw_piece();
            }

            self.con.gotoxy(1, 20);
            self.con.textcolor(4);
            match self.status() {
                WIN => {
                    self.emit(1, 20, WIN_MESSAGE);
                    if self.interactive {
                        self.con.getch();
                    }
                }
                LOSE => {
                    self.emit(1, 20, LOSE_MESSAGE);
                    if self.interactive {
                        self.con.getch();
                    }
                }
                DRAW => {
                    self.emit(1, 20, DRAW_MESSAGE);
                    if self.interactive {
                        self.con.getch();
                    }
                }
                _ => {
                    if self.computer_move() {
                        self.con.gotoxy(1, 20);
                        self.draw_piece();
                        self.con.gotoxy(1, 20);
                        match self.status() {
                            WIN => {
                                self.emit(1, 20, WIN_MESSAGE);
                                if self.interactive {
                                    self.con.getch();
                                }
                            }
                            LOSE => {
                                self.emit(1, 20, LOSE_MESSAGE);
                                if self.interactive {
                                    self.con.getch();
                                }
                            }
                            DRAW => {
                                self.emit(1, 20, DRAW_MESSAGE);
                                if self.interactive {
                                    self.con.getch();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            if !self.interactive && self.plies > 60 {
                return;
            }
        }
    }
}

fn y_base(x: i32, xsquare: &Board) -> i32 {
    if x < 0 || x as usize >= X_BOARD {
        return -1;
    }
    xsquare
        .iter()
        .position(|row| row[x as usize] == BASE)
        .map_or(-1, |y| y as i32)
}

fn make_move(who: i32, x: i32, xsquare: &mut Board) {
    let y = y_base(x, xsquare);
    if y < 0 {
        return;
    }
    xsquare[y as usize][x as usize] = who;
    if y > 0 {
        xsquare[(y - 1) as usize][x as usize] = BASE;
    }
}

fn main() -> libcaca::Result<()> {
    let con = Conio::new()?;
    let interactive = matches!(
        con.display().driver(),
        libcaca::Driver::Terminal | libcaca::Driver::Win32
    );
    let mut game = Game {
        con,
        mv: [3, 4, 2, 5, 1, 6, 0, 7],
        col: [-1; X_BOARD],
        square: [[EMPTY; X_BOARD]; Y_BOARD],
        interactive,
        plies: 0,
    };
    // Pre-fill the bottom row like init() will; play() starts clean.
    game.init();
    game.play();
    Ok(())
}
