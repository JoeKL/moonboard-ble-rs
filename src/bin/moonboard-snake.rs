use std::{
    collections::VecDeque,
    env,
    io::{stdout, Write},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use moonboard_ble::{coordinate_to_index, CoordinateOrder, Hold, HoldRole, IndexBase, MoonBoard};

const WIDTH: i8 = 11;
const HEIGHT: i8 = 18;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    const fn delta(self) -> (i8, i8) {
        match self {
            Self::Up => (0, 1),
            Self::Down => (0, -1),
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
        }
    }

    const fn opposite(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Up, Self::Down)
                | (Self::Down, Self::Up)
                | (Self::Left, Self::Right)
                | (Self::Right, Self::Left)
        )
    }
}

struct Game {
    snake: VecDeque<(i8, i8)>,
    direction: Direction,
    next_direction: Direction,
    food: (i8, i8),
    score: usize,
    rng: u64,
}

impl Game {
    fn new(seed: u64) -> Self {
        let mut game = Self {
            snake: VecDeque::from([(5, 9), (4, 9), (3, 9)]),
            direction: Direction::Right,
            next_direction: Direction::Right,
            food: (0, 0),
            score: 0,
            rng: seed,
        };
        game.place_food();
        game
    }

    fn turn(&mut self, direction: Direction) {
        if !self.direction.opposite(direction) {
            self.next_direction = direction;
        }
    }

    fn step(&mut self) -> bool {
        self.direction = self.next_direction;
        let (dx, dy) = self.direction.delta();
        let (x, y) = self.snake[0];
        let head = ((x + dx).rem_euclid(WIDTH), (y + dy).rem_euclid(HEIGHT));

        let growing = head == self.food;
        let occupied = if growing {
            self.snake.len()
        } else {
            self.snake.len() - 1
        };
        if self.snake.iter().take(occupied).any(|&part| part == head) {
            return false;
        }

        self.snake.push_front(head);
        if growing {
            self.score += 1;
            self.place_food();
        } else {
            self.snake.pop_back();
        }
        true
    }

    fn place_food(&mut self) {
        self.rng = self
            .rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let cell_count = WIDTH as usize * HEIGHT as usize;
        let start = (self.rng % cell_count as u64) as usize;
        for offset in 0..cell_count {
            let position = (start + offset) % cell_count;
            let cell = (
                (position % WIDTH as usize) as i8,
                (position / WIDTH as usize) as i8,
            );
            if !self.snake.contains(&cell) {
                self.food = cell;
                return;
            }
        }
    }

    fn holds(&self) -> Result<Vec<Hold>> {
        let mut holds = Vec::with_capacity(self.snake.len() + 1);
        for (position, &(column, row)) in self.snake.iter().enumerate() {
            holds.push(Hold {
                index: led_index(column, row)?,
                role: if position == 0 {
                    HoldRole::Start
                } else {
                    HoldRole::Right
                },
            });
        }
        holds.push(Hold {
            index: led_index(self.food.0, self.food.1)?,
            role: HoldRole::End,
        });
        Ok(holds)
    }

    fn render(&self) -> Result<()> {
        let mut output = stdout();
        execute!(output, MoveTo(0, 0), Clear(ClearType::All))?;
        write!(
            output,
            "MoonBoard Snake — arrows/WASD move, q/Esc quits\r\nScore: {}\r\n\r\n   A B C D E F G H I J K\r\n",
            self.score
        )?;
        for row in (0..HEIGHT).rev() {
            write!(output, "{:>2} ", row + 1)?;
            for column in 0..WIDTH {
                let cell = (column, row);
                let symbol = if cell == self.snake[0] {
                    '@'
                } else if cell == self.food {
                    '*'
                } else if self.snake.contains(&cell) {
                    'o'
                } else {
                    '.'
                };
                write!(output, "{symbol} ")?;
            }
            write!(output, "\r\n")?;
        }
        output.flush()?;
        Ok(())
    }
}

fn led_index(column: i8, row: i8) -> Result<u16> {
    let coordinate = format!("{}{}", char::from(b'A' + column as u8), row + 1);
    coordinate_to_index(
        &coordinate,
        CoordinateOrder::ColumnSerpentine,
        IndexBase::Zero,
    )
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        if let Err(error) = execute!(stdout(), EnterAlternateScreen, Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(error.into());
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let selector = args.next().context(
        "usage: moonboard-snake <device name or MAC> [tick-ms]\n\
         example: moonboard-snake MoonBoard 350",
    )?;
    let tick_ms = args
        .next()
        .map(|value| value.parse())
        .transpose()
        .context("tick-ms must be a positive integer")?
        .unwrap_or(350);
    if tick_ms == 0 {
        anyhow::bail!("tick-ms must be greater than zero");
    }

    let board = MoonBoard::connect(&selector, Duration::from_secs(10)).await?;
    let seed = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as u64;
    let mut game = Game::new(seed);
    let terminal = TerminalGuard::enter()?;
    game.render()?;
    board.light(&game.holds()?).await?;

    let mut quit = false;
    while !quit {
        let deadline = Instant::now() + Duration::from_millis(tick_ms);
        while Instant::now() < deadline {
            let wait = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50));
            if event::poll(wait)? {
                if let Event::Key(key) = event::read()? {
                    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                        continue;
                    }
                    match key.code {
                        KeyCode::Up | KeyCode::Char('w' | 'W') => game.turn(Direction::Up),
                        KeyCode::Down | KeyCode::Char('s' | 'S') => game.turn(Direction::Down),
                        KeyCode::Left | KeyCode::Char('a' | 'A') => game.turn(Direction::Left),
                        KeyCode::Right | KeyCode::Char('d' | 'D') => game.turn(Direction::Right),
                        KeyCode::Char('q') | KeyCode::Esc => quit = true,
                        _ => {}
                    }
                }
            }
        }

        if quit || !game.step() {
            break;
        }
        game.render()?;
        board.light(&game.holds()?).await?;
    }

    drop(terminal);
    println!("Game over. Score: {}", game.score);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_moves_and_cannot_reverse() {
        let mut game = Game::new(1);
        game.food = (0, 0);
        game.turn(Direction::Left);
        assert!(game.step());
        assert_eq!(game.snake[0], (6, 9));
        assert_eq!(game.snake.len(), 3);
    }

    #[test]
    fn snake_wraps_at_every_border() {
        let mut game = Game::new(1);
        game.snake = VecDeque::from([(10, 5)]);
        game.direction = Direction::Right;
        game.next_direction = Direction::Right;
        game.food = (5, 5);
        assert!(game.step());
        assert_eq!(game.snake[0], (0, 5));

        game.snake = VecDeque::from([(0, 5)]);
        game.direction = Direction::Left;
        game.next_direction = Direction::Left;
        assert!(game.step());
        assert_eq!(game.snake[0], (10, 5));

        game.snake = VecDeque::from([(5, 17)]);
        game.direction = Direction::Up;
        game.next_direction = Direction::Up;
        assert!(game.step());
        assert_eq!(game.snake[0], (5, 0));

        game.snake = VecDeque::from([(5, 0)]);
        game.direction = Direction::Down;
        game.next_direction = Direction::Down;
        assert!(game.step());
        assert_eq!(game.snake[0], (5, 17));
    }
}
