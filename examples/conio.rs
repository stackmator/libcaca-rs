//! Port of libcaca's `examples/conio.c`: 4-in-a-line via [`Conio`].
//!
//! Cheok Yan Cheng's game with Sam Hocevar's AI (`bestmove` depth search).
//! Move with the arrow keys, select with Space, Escape exits. The C version
//! reads DOS scan codes (75/77) and writes the board with raw `printf`
//! (which lands on the same DOS screen); here arrow keys arrive as key
//! codes and everything renders through [`Conio`], which looks identical.

// The game logic addresses the board by x/y coordinates exactly like the
// C version; iterator rewrites would obscure the correspondence.
#![allow(clippy::needless_range_loop)]

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

const MOVE_ORDER: [usize; 8] = [3, 4, 2, 5, 1, 6, 0, 7];

type Board = [[i32; X_BOARD]; Y_BOARD];

struct Game {
    con: Conio,
    col: [i32; X_BOARD],
    square: Board,
}

impl Game {
    fn computer_move(&mut self) -> bool {
        if self.is_full() {
            return false;
        }
        let x_best = self.bestmove();
        self.con.gotoxy(52, 2);
        self.con.cprintf(format_args!(
            "x:{}, y:{}\n",
            x_best + 1,
            Y_BOARD as i32 - y_base(x_best, &self.square)
        ));
        let mut square = self.square;
        make_move(COMPUTER, x_best, &mut square);
        self.square = square;
        true
    }

    fn double_head(who: i32, xsquare: &Board) -> bool {
        for y in 0..Y_BOARD {
            for x in 0..X_BOARD - 4 {
                if xsquare[y][x] == BASE
                    && xsquare[y][x + 1] == who
                    && xsquare[y][x + 2] == who
                    && xsquare[y][x + 3] == who
                    && xsquare[y][x + 4] == BASE
                {
                    return true;
                }
            }
        }
        false
    }

    fn human_move(&mut self, x: i32) -> bool {
        self.con.gotoxy(1, 20);
        if x < 0 || x >= X_BOARD as i32 {
            return false;
        }
        if y_base(x, &self.square) == -1 {
            return false;
        }
        if self.is_full() {
            return false;
        }
        self.con.gotoxy(52, 5);
        self.con.cprintf(format_args!(
            "x:{}, y:{}\n",
            x + 1,
            Y_BOARD as i32 - y_base(x, &self.square)
        ));
        let mut square = self.square;
        make_move(HUMAN, x, &mut square);
        self.square = square;
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
                // Horizontal.
                if x + 3 < X_BOARD
                    && xsquare[y][x] == who
                    && xsquare[y][x + 1] == who
                    && xsquare[y][x + 2] == who
                    && xsquare[y][x + 3] == who
                {
                    return true;
                }
                // Vertical.
                if y + 3 < Y_BOARD
                    && xsquare[y][x] == who
                    && xsquare[y + 1][x] == who
                    && xsquare[y + 2][x] == who
                    && xsquare[y + 3][x] == who
                {
                    return true;
                }
                // Downstair diagonal.
                if x + 3 < X_BOARD
                    && y + 3 < Y_BOARD
                    && xsquare[y][x] == who
                    && xsquare[y + 1][x + 1] == who
                    && xsquare[y + 2][x + 2] == who
                    && xsquare[y + 3][x + 3] == who
                {
                    return true;
                }
                // Upstair diagonal.
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

    fn two_snake(who: i32, x: usize, xsquare: &Board) -> bool {
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
        for i in 0..Y_BOARD - 1 {
            if n[i] && n[i + 1] {
                return true;
            }
        }
        false
    }

    fn bestmove(&mut self) -> i32 {
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

        for i in 0..X_BOARD {
            if y_base(MOVE_ORDER[i] as i32, &self.square) != -1
                && self.col[MOVE_ORDER[i]] == COMPUTER
            {
                let mut xsquare = self.square;
                make_move(COMPUTER, MOVE_ORDER[i] as i32, &mut xsquare);
                if Self::x_won(HUMAN, &xsquare) == -1 {
                    self.con
                        .cprintf(format_args!("Computer Previous Depth : +2\n"));
                    return MOVE_ORDER[i] as i32;
                }
            }
        }

        if Self::x_double_head(COMPUTER, &self.square) != -1 {
            let col = Self::x_double_head(COMPUTER, &self.square);
            let mut xsquare = self.square;
            make_move(COMPUTER, col, &mut xsquare);
            if Self::x_won(HUMAN, &xsquare) == -1 {
                self.con
                    .cprintf(format_args!("Computer Previous Depth : +3\n"));
                return Self::x_double_head(COMPUTER, &self.square);
            }
        }

        if Self::x_double_head(HUMAN, &self.square) != -1 {
            let col = Self::x_double_head(HUMAN, &self.square);
            let mut xsquare = self.square;
            make_move(COMPUTER, col, &mut xsquare);
            if Self::x_won(HUMAN, &xsquare) == -1 {
                self.con
                    .cprintf(format_args!("Computer Previous Depth : -3\n"));
                return Self::x_double_head(HUMAN, &self.square);
            }
        }

        let square = self.square;
        let snake = self.x_two_snake(COMPUTER, &square);
        if snake != -1 {
            let mut xsquare = self.square;
            make_move(COMPUTER, snake, &mut xsquare);
            if Self::x_won(HUMAN, &xsquare) == -1 {
                self.con
                    .cprintf(format_args!("Computer Previous Depth : +4\n"));
                return snake;
            }
        }
        let square = self.square;
        if self.x_two_snake(HUMAN, &square) != -1 {
            let col = self.x_two_snake(HUMAN, &square);
            let mut xsquare = self.square;
            make_move(COMPUTER, col, &mut xsquare);
            if Self::x_won(HUMAN, &xsquare) == -1 {
                self.con
                    .cprintf(format_args!("Computer Previous Depth : -4\n"));
                let square = self.square;
                return self.x_two_snake(HUMAN, &square);
            }
        }

        let mut n = [0i32; X_BOARD];
        Self::gen_num_win(&self.square, &mut n);
        sorting(&mut n);

        for i in 0..X_BOARD {
            if y_base(n[i], &self.square) != -1 {
                let mut xsquare = self.square;
                make_move(COMPUTER, n[i], &mut xsquare);
                if Self::x_won(HUMAN, &xsquare) == -1 {
                    self.con
                        .cprintf(format_args!("Computer Previous Depth : +5\n"));
                    return n[i];
                }
            }
        }

        for i in 0..X_BOARD {
            if y_base(MOVE_ORDER[i] as i32, &self.square) != -1 {
                self.con
                    .cprintf(format_args!("Computer Previous Depth : +0\n"));
                return MOVE_ORDER[i] as i32;
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
        for x in 0..X_BOARD as i32 {
            if y_base(x, xsquare) != -1 {
                let mut xxsquare = *xsquare;
                make_move(who, x, &mut xxsquare);
                if Self::double_head(who, &xxsquare) {
                    return x;
                }
            }
        }
        -1
    }

    fn x_two_snake(&mut self, who: i32, xsquare: &Board) -> i32 {
        for x in 0..X_BOARD {
            if y_base(MOVE_ORDER[x] as i32, xsquare) != -1 {
                let mut xxsquare = *xsquare;
                make_move(who, MOVE_ORDER[x] as i32, &mut xxsquare);
                for dx in 0..X_BOARD {
                    if Self::two_snake(who, MOVE_ORDER[dx], &xxsquare)
                        && self.col[MOVE_ORDER[dx]] != who
                    {
                        if who == COMPUTER {
                            self.col[MOVE_ORDER[dx]] = who;
                        }
                        return MOVE_ORDER[x] as i32;
                    }
                }
            }
        }
        -1
    }

    fn x_won(who: i32, xsquare: &Board) -> i32 {
        for x in 0..X_BOARD as i32 {
            if y_base(x, xsquare) != -1 {
                let mut xxsquare = *xsquare;
                make_move(who, x, &mut xxsquare);
                if Self::is_won(who, &xxsquare) {
                    return x;
                }
            }
        }
        -1
    }

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
        self.con.gotoxy(1, 1);
        for _ in 0..8 {
            self.con.cprintf(format_args!(
                "+-----+-----+-----+-----+-----+-----+-----+-----+\n"
            ));
            self.con.cprintf(format_args!(
                "|     |     |     |     |     |     |     |     |\n"
            ));
        }
        self.con.cprintf(format_args!(
            "+-----+-----+-----+-----+-----+-----+-----+-----+\n"
        ));
        self.con
            .cprintf(format_args!("ARROW:move\tSPACE:select\tESC:exit\n"));
        self.con.textcolor(1);
        self.con.gotoxy(44, 20);
        self.con.cprintf(format_args!("4 In Line\n"));
        self.con.gotoxy(44, 21);
        self.con
            .cprintf(format_args!("ver Beta by Cheok Yan Cheng\n"));
        self.con.gotoxy(44, 22);
        self.con
            .cprintf(format_args!("E-mail : yccheok@yahoo.com\n"));
        self.con.gotoxy(44, 23);
        self.con
            .cprintf(format_args!("Web Site: www.geocities.com/yccheok\n"));
        self.con.gotoxy(44, 24);
        self.con.cprintf(format_args!("Source code included!\n"));
        self.con.gotoxy(1, 22);
        self.con.cprintf(format_args!("Human's Piece is O\n"));
        self.con.gotoxy(1, 23);
        self.con.cprintf(format_args!("Computer's Piece is X\n"));
        self.con.gotoxy(52, 1);
        self.con.cprintf(format_args!("Computer Move :\n"));
        self.con.gotoxy(52, 4);
        self.con.cprintf(format_args!("Human Move :\n"));
    }

    fn draw_piece(&mut self) {
        for x in 0..X_BOARD {
            for y in 0..Y_BOARD {
                if self.square[y][x] == HUMAN {
                    self.con.gotoxy(x as i32 * 6 + 4, y as i32 * 2 + 2);
                    self.con.textcolor(1);
                    self.con.cprintf(format_args!("O\n"));
                } else if self.square[y][x] == COMPUTER {
                    self.con.gotoxy(x as i32 * 6 + 4, y as i32 * 2 + 2);
                    self.con.textcolor(4);
                    self.con.cprintf(format_args!("X\n"));
                }
            }
        }
    }

    fn get_human_move(&mut self) {
        let mut x = 3i32;
        loop {
            self.con.gotoxy(x * 6 + 4, 2);
            match self.con.getch() {
                // DOS scan codes 75/77 arrive as key codes here.
                c if c == key::LEFT || c == 75 => {
                    if x > 0 {
                        x -= 1;
                    }
                }
                c if c == key::RIGHT || c == 77 => {
                    if x < X_BOARD as i32 - 1 {
                        x += 1;
                    }
                }
                c if c == key::ESCAPE || c == 27 => {
                    self.con.textcolor(7);
                    self.con.textbackground(0);
                    self.con.clrscr();
                    self.con.cprintf(format_args!(
                        "Thank You For Playing 4 in line by Cheok Yan Cheng!\n"
                    ));
                    std::process::exit(0);
                }
                c if c == b' ' as i32 || c == 32 => {
                    if self.human_move(x) {
                        self.draw_piece();
                        return;
                    }
                    self.con.gotoxy(1, 20);
                    self.con.textcolor(4);
                    self.con.cprintf(format_args!("OOPs! Wrong Move! \n"));
                }
                _ => {}
            }
        }
    }

    fn init(&mut self) {
        for x in 0..X_BOARD {
            for y in 0..Y_BOARD - 1 {
                self.square[y][x] = EMPTY;
            }
            self.square[7][x] = BASE;
            self.col[x] = -1;
        }
    }
}

fn y_base(x: i32, xsquare: &Board) -> i32 {
    if x < 0 || x >= X_BOARD as i32 {
        return -1;
    }
    for y in 0..Y_BOARD {
        if xsquare[y][x as usize] == BASE {
            return y as i32;
        }
    }
    -1
}

fn make_move(who: i32, x: i32, xsquare: &mut Board) {
    let y = y_base(x, xsquare);
    xsquare[y as usize][x as usize] = who;
    if y > 0 {
        xsquare[y as usize - 1][x as usize] = BASE;
    }
}

fn sorting(n: &mut [i32; X_BOARD]) {
    let mut store = [0i32; X_BOARD];
    for j in 0..X_BOARD {
        let alpha = n.iter().copied().max().unwrap_or(0);
        for i in 0..X_BOARD {
            if n[MOVE_ORDER[i]] == alpha {
                store[j] = MOVE_ORDER[i] as i32;
                n[MOVE_ORDER[i]] = -1;
                break;
            }
        }
    }
    *n = store;
}

fn main() {
    let mut game = Game {
        con: Conio::new().unwrap(),
        col: [-1; X_BOARD],
        square: [[EMPTY; X_BOARD]; Y_BOARD],
    };
    // The bottom row starts as BASE cells, like the C initializer.
    for x in 0..X_BOARD {
        game.square[7][x] = BASE;
    }

    let mut myturn = true;
    game.draw_board();
    // The C version seeds libc rand here; our generator needs no seeding.
    loop {
        match game.status() {
            WIN | LOSE | DRAW => {
                game.init();
                game.draw_board();
                if myturn {
                    let mut square = game.square;
                    make_move(COMPUTER, 2 + rand(0, 4), &mut square);
                    game.square = square;
                }
                myturn = !myturn;
                game.draw_piece();
            }
            _ => {}
        }
        game.con.textcolor(4);
        game.con.gotoxy(1, 20);
        game.con.cprintf(format_args!("Your Turn, Please.\n"));
        game.get_human_move();
        game.con.gotoxy(1, 20);
        game.con.textcolor(4);
        match game.status() {
            WIN => {
                game.con.cprintf(format_args!("{WIN_MESSAGE}"));
                game.con.getch();
            }
            LOSE => {
                game.con.cprintf(format_args!("{LOSE_MESSAGE}"));
                game.con.getch();
            }
            DRAW => {
                game.con.cprintf(format_args!("{DRAW_MESSAGE}"));
                game.con.getch();
            }
            _ => {
                if game.computer_move() {
                    game.con.gotoxy(1, 20);
                    game.draw_piece();
                    game.con.gotoxy(1, 20);
                    match game.status() {
                        WIN => {
                            game.con.cprintf(format_args!("{WIN_MESSAGE}"));
                            game.con.getch();
                        }
                        LOSE => {
                            game.con.cprintf(format_args!("{LOSE_MESSAGE}"));
                            game.con.getch();
                        }
                        DRAW => {
                            game.con.cprintf(format_args!("{DRAW_MESSAGE}"));
                            game.con.getch();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
