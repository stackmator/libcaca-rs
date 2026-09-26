//! Port of libcaca's `examples/conio-snake.cpp`.
//!
//! A snake game on the DOS-style [`Conio`] console. Steer with `o`/`p`
//! (left/right) and `a`/`z` (up/down), `x` quits. Headless runs play a short
//! session with no delays and exit.

use libcaca::conio::Conio;
use libcaca::{rand, Driver};

const MAXROW: usize = 15;
const MAXCOL: usize = 77;
const SNAKE_START_COL: i32 = 33;
const SNAKE_START_ROW: i32 = 7;
const UP_KEY: u8 = b'a';
const DOWN_KEY: u8 = b'z';
const LEFT_KEY: u8 = b'o';
const RIGHT_KEY: u8 = b'p';
const PAUSE_LENGTH: i32 = 500000;

const YELLOW: u8 = 14;
const LIGHTRED: u8 = 12;
const LIGHTBLUE: u8 = 9;
const WHITE: u8 = 15;
const LIGHTCYAN: u8 = 11;
const LIGHTGREEN: u8 = 10;
const LIGHTMAGENTA: u8 = 13;

struct Game {
    con: Conio,
    score: i32,
    snake_length: usize,
    speed: i32,
    obstacles: i32,
    level: i32,
    firstpress: bool,
    high_score: i32,
    grid: [[u8; MAXCOL]; MAXROW],
    direction: u8,
    snake: [(i32, i32); 100],
    interactive: bool,
    frames: u32,
}

impl Game {
    fn setup_level(&mut self) {
        self.snake_length = (self.level + 4) as usize;
        self.direction = RIGHT_KEY;
        self.firstpress = true;

        for row in self.grid.iter_mut() {
            for cell in row.iter_mut() {
                *cell = b' ';
            }
        }

        for i in 0..self.obstacles * 2 {
            let row = rand(0, MAXROW as i32) as usize;
            let col = rand(0, MAXCOL as i32) as usize;
            self.grid[row][col] = if i < self.obstacles { b'x' } else { b'.' };
        }

        for (i, s) in self.snake.iter_mut().enumerate().take(self.snake_length) {
            *s = (SNAKE_START_ROW, SNAKE_START_COL + i as i32);
        }

        self.draw_line(1, 1);
        for row in 0..MAXROW {
            self.con.gotoxy(1, row as i32 + 2);
            self.con.textcolor(LIGHTBLUE);
            self.con.cprintf(format_args!("|"));
            self.con.textcolor(WHITE);
            for col in 0..MAXCOL {
                self.con
                    .cprintf(format_args!("{}", self.grid[row][col] as char));
            }
            self.con.textcolor(LIGHTBLUE);
            self.con.cprintf(format_args!("|"));
        }
        self.draw_line(1, MAXROW as i32 + 2);

        self.show_score();
        self.con.gotoxy(2, MAXROW as i32 + 5);
        self.con.textcolor(LIGHTRED);
        self.con.cprintf(format_args!(
            "~~ SNAKE GAME~~ Left: {}, Right: {}, Up: {}, Down: {}, Exit: x. Any key to start.",
            LEFT_KEY as char, RIGHT_KEY as char, UP_KEY as char, DOWN_KEY as char
        ));
    }

    fn draw_line(&mut self, col: i32, row: i32) {
        self.con.gotoxy(col, row);
        self.con.textcolor(LIGHTBLUE);
        for _ in 0..MAXCOL + 2 {
            self.con.cprintf(format_args!("="));
        }
    }

    fn show_score(&mut self) {
        self.con.textcolor(LIGHTCYAN);
        self.con.gotoxy(2, MAXROW as i32 + 3);
        self.con.cprintf(format_args!("Level: {:05}", self.level));
        self.con.gotoxy(40, MAXROW as i32 + 3);
        self.con.textcolor(LIGHTGREEN);
        self.con.cprintf(format_args!("Score: {:05}", self.score));
        self.con.gotoxy(60, MAXROW as i32 + 3);
        self.con.textcolor(LIGHTMAGENTA);
        self.con
            .cprintf(format_args!("High Score: {:05}", self.high_score));
    }

    fn add_segment(&mut self) {
        // Index `snake_length` is the scratch slot past the tail.
        let (row, col) = self.snake[self.snake_length];
        self.snake[self.snake_length] = match self.direction {
            RIGHT_KEY => (row, col + 1),
            LEFT_KEY => (row, col - 1),
            UP_KEY => (row - 1, col),
            _ => (row + 1, col),
        };
    }

    fn play(&mut self) {
        let mut keypress: i32 = 0;

        loop {
            self.obstacles = 4;
            self.level = 1;
            self.score = 0;
            self.speed = 14;
            self.setup_level();

            loop {
                if self.interactive {
                    self.con.delay((self.speed * PAUSE_LENGTH / 50000) as u32);
                }

                if self.con.kbhit() {
                    keypress = self.con.getch();
                    let k = keypress as u8;
                    if k == RIGHT_KEY || k == LEFT_KEY || k == UP_KEY || k == DOWN_KEY {
                        self.direction = k;
                    }
                }

                self.add_segment();

                let (row, col) = (self.snake[0].0, self.snake[0].1);
                self.con.gotoxy(col, row);
                self.con.cprintf(format_args!(" "));
                for i in 1..=self.snake_length {
                    self.snake[i - 1] = self.snake[i];
                }
                self.con.textcolor(YELLOW);
                for i in 0..=self.snake_length {
                    self.con.gotoxy(self.snake[i].1, self.snake[i].0);
                    self.con.cprintf(format_args!("O"));
                }
                self.con.gotoxy(1, 1);

                if self.firstpress {
                    if self.interactive {
                        while !self.con.kbhit() {}
                    }
                    self.firstpress = false;
                }

                let (hr, hc) = self.snake[self.snake_length - 1];
                if hr > MAXROW as i32 + 1
                    || hr <= 1
                    || hc > MAXCOL as i32 + 1
                    || hc <= 1
                    || self.grid[(hr - 2) as usize][(hc - 2) as usize] == b'x'
                {
                    keypress = b'x' as i32;
                }
                for i in 0..self.snake_length - 1 {
                    if self.snake[self.snake_length - 1] == self.snake[i] {
                        keypress = b'x' as i32;
                        break;
                    }
                }
                // NB: the C version indexes out of bounds after a wall hit;
                // only sample the grid when the head is inside it.
                if hr >= 2
                    && hr < MAXROW as i32 + 2
                    && hc >= 2
                    && hc < MAXCOL as i32 + 2
                    && self.grid[(hr - 2) as usize][(hc - 2) as usize] == b'.'
                {
                    self.score += self.snake_length as i32 * self.obstacles;
                    self.show_score();
                    // The C version can overflow its static array; cap growth.
                    if self.snake_length + 1 < 100 {
                        self.snake_length += 1;
                        self.add_segment();
                    }
                    if self.snake_length == ((self.level + 3) * 2) as usize {
                        self.score += self.level * 1000;
                        self.obstacles += 2;
                        self.level += 1;
                        if self.level % 5 == 0 && self.speed > 1 {
                            self.speed -= 1;
                        }
                        self.setup_level();
                    }
                }

                if keypress == b'x' as i32 {
                    break;
                }

                if !self.interactive {
                    self.frames += 1;
                    if self.frames >= 200 {
                        break;
                    }
                }
            }

            if self.score > self.high_score {
                self.high_score = self.score;
            }
            self.show_score();
            self.con.gotoxy(30, 6);
            self.con.textcolor(LIGHTRED);
            self.con.cprintf(format_args!("G A M E   O V E R"));
            self.con.gotoxy(30, 9);
            self.con.textcolor(YELLOW);
            self.con.cprintf(format_args!("Another Game (y/n)? "));

            if self.interactive {
                loop {
                    keypress = self.con.getch();
                    if keypress == b'y' as i32 || keypress == b'n' as i32 {
                        break;
                    }
                }
            } else {
                keypress = b'n' as i32;
            }

            if keypress != b'y' as i32 {
                break;
            }
        }
    }
}

fn main() -> libcaca::Result<()> {
    let con = Conio::new()?;
    let interactive = matches!(con.display().driver(), Driver::Terminal | Driver::Win32);
    let mut game = Game {
        con,
        score: 0,
        snake_length: 0,
        speed: 14,
        obstacles: 4,
        level: 1,
        firstpress: true,
        high_score: 0,
        grid: [[b' '; MAXCOL]; MAXROW],
        direction: RIGHT_KEY,
        snake: [(0, 0); 100],
        interactive,
        frames: 0,
    };
    game.play();
    Ok(())
}
