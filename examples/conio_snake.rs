//! Port of libcaca's `examples/conio-snake.cpp`.
//!
//! The same snake game as [`snake`](snake.rs), but written against the
//! DOS-style [`Conio`] API exactly like the C++ original: `a`/`z` for
//! up/down, `o`/`p` for left/right, `x` to quit. Kept as a duplicate on
//! purpose — it exercises the conio layer rather than the canvas API.

use libcaca::conio::Conio;
use libcaca::rand;

// DOS palette indices.
const YELLOW: u8 = 14;
const WHITE: u8 = 15;
const LIGHTBLUE: u8 = 9;
const LIGHTCYAN: u8 = 11;
const LIGHTRED: u8 = 12;
const LIGHTGREEN: u8 = 10;
const LIGHTMAGENTA: u8 = 13;

const MAXROW: i32 = 15;
const MAXCOL: i32 = 77;
const SNAKE_START_COL: i32 = 33;
const SNAKE_START_ROW: i32 = 7;
const UP_KEY: i32 = b'a' as i32;
const DOWN_KEY: i32 = b'z' as i32;
const LEFT_KEY: i32 = b'o' as i32;
const RIGHT_KEY: i32 = b'p' as i32;
const PAUSE_LENGTH: i32 = 500_000;

struct Game {
    con: Conio,
    score: i32,
    snake_length: i32,
    speed: i32,
    obstacles: i32,
    level: i32,
    firstpress: bool,
    high_score: i32,
    screen_grid: [[u8; MAXCOL as usize]; MAXROW as usize],
    direction: i32,
    snake: [(i32, i32); 100],
}

impl Game {
    fn draw_line(&mut self, col: i32, row: i32) {
        self.con.gotoxy(col, row);
        self.con.textcolor(LIGHTBLUE);
        for _ in 0..MAXCOL + 2 {
            self.con.cprintf(format_args!("="));
        }
    }

    fn show_score(&mut self) {
        self.con.textcolor(LIGHTCYAN);
        self.con.gotoxy(2, MAXROW + 3);
        self.con.cprintf(format_args!("Level: {:05}", self.level));
        self.con.gotoxy(40, MAXROW + 3);
        self.con.textcolor(LIGHTGREEN);
        self.con.cprintf(format_args!("Score: {:05}", self.score));
        self.con.gotoxy(60, MAXROW + 3);
        self.con.textcolor(LIGHTMAGENTA);
        self.con
            .cprintf(format_args!("High Score: {:05}", self.high_score));
    }

    fn add_segment(&mut self) {
        let (row, col) = self.snake[self.snake_length as usize - 1];
        let next = match self.direction {
            RIGHT_KEY => (row, col + 1),
            LEFT_KEY => (row, col - 1),
            UP_KEY => (row - 1, col),
            _ => (row + 1, col),
        };
        self.snake[self.snake_length as usize] = next;
    }

    fn setup_level(&mut self) {
        // Set up globals for the new level.
        self.snake_length = self.level + 4;
        self.direction = RIGHT_KEY;
        self.firstpress = true;
        // Fill grid with blanks.
        for row in self.screen_grid.iter_mut() {
            for cell in row.iter_mut() {
                *cell = b' ';
            }
        }

        // Fill grid with Xs and food.
        for i in 0..self.obstacles * 2 {
            let row = rand(0, MAXROW) as usize;
            let col = rand(0, MAXCOL) as usize;
            self.screen_grid[row][col] = if i < self.obstacles { b'x' } else { b'.' };
        }

        // Create snake array of length snake_length.
        for i in 0..self.snake_length as usize {
            self.snake[i] = (SNAKE_START_ROW, SNAKE_START_COL + i as i32);
        }

        // Draw playing board.
        self.draw_line(1, 1);
        for row in 0..MAXROW {
            self.con.gotoxy(1, row + 2);
            self.con.textcolor(LIGHTBLUE);
            self.con.cprintf(format_args!("|"));
            self.con.textcolor(WHITE);
            for col in 0..MAXCOL {
                let c = self.screen_grid[row as usize][col as usize];
                self.con.cprintf(format_args!("{}", c as char));
            }
            self.con.textcolor(LIGHTBLUE);
            self.con.cprintf(format_args!("|"));
        }
        self.draw_line(1, MAXROW + 2);

        self.show_score();
        self.con.gotoxy(2, MAXROW + 5);
        self.con.textcolor(LIGHTRED);
        self.con.cprintf(format_args!(
            "~~ SNAKE GAME~~ Left: {}, Right: {}, Up: {}, Down: {}. Exit: x. Any key to start.",
            LEFT_KEY as u8 as char,
            RIGHT_KEY as u8 as char,
            UP_KEY as u8 as char,
            DOWN_KEY as u8 as char
        ));
    }
}

fn main() {
    let mut game = Game {
        con: Conio::new().unwrap(),
        score: 0,
        snake_length: 0,
        speed: 0,
        obstacles: 0,
        level: 0,
        firstpress: false,
        high_score: 0,
        screen_grid: [[b' '; MAXCOL as usize]; MAXROW as usize],
        direction: RIGHT_KEY,
        snake: [(0, 0); 100],
    };

    loop {
        // Restart game loop.
        game.obstacles = 4;
        game.level = 1;
        game.score = 0;
        game.speed = 14;

        game.setup_level();

        // Main loop.
        let mut keypress = 0;
        while keypress != b'x' as i32 {
            game.con.delay((game.speed * PAUSE_LENGTH / 50_000) as u32);

            // If key has been hit, then check it is a direction key - if
            // so, change direction.
            if game.con.kbhit() {
                keypress = game.con.getch();
                if keypress == RIGHT_KEY
                    || keypress == LEFT_KEY
                    || keypress == UP_KEY
                    || keypress == DOWN_KEY
                {
                    game.direction = keypress;
                }
            }

            // Add a segment to the end of the snake.
            game.add_segment();

            // Blank last segment of snake.
            let (row, col) = game.snake[0];
            game.con.gotoxy(col, row);
            game.con.cprintf(format_args!(" "));
            // ... and remove it from the array.
            for i in 1..=game.snake_length as usize {
                game.snake[i - 1] = game.snake[i];
            }
            // Display snake in yellow.
            game.con.textcolor(YELLOW);
            for i in 0..=game.snake_length as usize {
                let (row, col) = game.snake[i];
                game.con.gotoxy(col, row);
                game.con.cprintf(format_args!("O"));
            }
            // Keep cursor flashing in one place instead of following snake.
            game.con.gotoxy(1, 1);

            // If first press on each level, pause until a key is pressed.
            if game.firstpress {
                while !game.con.kbhit() {}
                game.firstpress = false;
            }

            let head = game.snake[game.snake_length as usize - 1];
            // Collision detection - walls (bad!), then obstacles (bad!).
            // `||` short-circuits left to right exactly like the C version,
            // so the grid is only indexed with in-range coordinates.
            if head.0 > MAXROW + 1
                || head.0 <= 1
                || head.1 > MAXCOL + 1
                || head.1 <= 1
                || game.screen_grid[(head.0 - 2) as usize][(head.1 - 2) as usize] == b'x'
            {
                keypress = b'x' as i32; // i.e. exit loop - game over
            }
            // Collision detection - snake (bad!).
            for i in 0..game.snake_length as usize - 1 {
                if head == game.snake[i] {
                    keypress = b'x' as i32; // i.e. exit loop - game over
                    break; // no need to check any more segments
                }
            }
            // Collision detection - food (good!). The C version indexes
            // the grid unconditionally (out of bounds on wall hits); only
            // probe it when the head is inside the walls.
            let in_bounds =
                (2..=MAXROW + 1).contains(&head.0) && (2..=MAXCOL + 1).contains(&head.1);
            if in_bounds && game.screen_grid[(head.0 - 2) as usize][(head.1 - 2) as usize] == b'.' {
                // Increase score and length of snake.
                game.score += game.snake_length * game.obstacles;
                game.show_score();
                game.snake_length += 1;
                game.add_segment();
                // If length of snake reaches certain size, onto next level.
                if game.snake_length == (game.level + 3) * 2 {
                    game.score += game.level * 1000;
                    game.obstacles += 2;
                    game.level += 1; // add to obstacles
                    if game.level % 5 == 0 && game.speed > 1 {
                        game.speed -= 1; // increase speed every 5 levels
                    }
                    game.setup_level(); // display next level
                }
            }
        }

        // Game over message.
        if game.score > game.high_score {
            game.high_score = game.score;
        }
        game.show_score();
        game.con.gotoxy(30, 6);
        game.con.textcolor(LIGHTRED);
        game.con.cprintf(format_args!("G A M E   O V E R"));
        game.con.gotoxy(30, 9);
        game.con.textcolor(YELLOW);
        game.con.cprintf(format_args!("Another Game (y/n)? "));
        loop {
            keypress = game.con.getch();
            if keypress == b'y' as i32 || keypress == b'n' as i32 {
                break;
            }
        }

        if keypress != b'y' as i32 {
            break;
        }
    }
}
