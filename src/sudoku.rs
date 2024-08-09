use anyhow::Result;
use rand::{prelude::SliceRandom, seq::IteratorRandom};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const SIZE: usize = 9;
const SUBGRID_SIZE: usize = 3;

pub const MAX_CHECKS: usize = 3;
pub const MAX_HINTS: usize = 3;

#[derive(Default)]
pub struct Sudoku {
    grid: [[Cell; SIZE]; SIZE],
    solution: [[Cell; SIZE]; SIZE],
    state: GameState,
    movements: Vec<Move>,
    start: Option<Instant>,
    elapsed: Duration,
    difficulty: Difficulty,
    checks: usize,
    hints: usize,
}

#[derive(Serialize, Deserialize)]
struct Save {
    grid: [[Cell; SIZE]; SIZE],
    solution: [[Cell; SIZE]; SIZE],
    difficulty: Difficulty,
    elapsed: u64,
    checks: usize,
    hints: usize,
}

struct Move {
    x: usize,
    y: usize,
    old: u8,
}

#[derive(Clone, Default)]
pub struct Board {
    grid: [[u8; SIZE]; SIZE],
}

#[derive(Serialize, Deserialize, Copy, Clone, Default)]
pub struct Cell {
    pub value: u8,
    pub locked: bool,
    pub checked: Option<bool>,
}

impl Cell {
    pub fn new(value: u8) -> Self {
        Self {
            value,
            locked: value != 0,
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, Default)]
enum GameState {
    #[default]
    Running,
    Paused,
    Solved,
    Won,
}

#[derive(Serialize, Deserialize, Default, Clone, Copy)]
pub enum Difficulty {
    #[default]
    Easy,
    Medium,
    Hard,
    Expert,
}

impl Difficulty {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Medium => "Medium",
            Self::Hard => "Hard",
            Self::Expert => "Expert",
        }
    }

    pub fn num_holes(&self) -> usize {
        let rng = &mut rand::thread_rng();
        match self {
            Difficulty::Easy => (45..50).choose(rng).unwrap(),
            Difficulty::Medium => (50..55).choose(rng).unwrap(),
            Difficulty::Hard => (55..60).choose(rng).unwrap(),
            Difficulty::Expert => (60..65).choose(rng).unwrap(),
        }
    }
}

impl Sudoku {
    pub fn generate(difficulty: Difficulty) -> Self {
        let solution = Board::generate();
        let puzzle = solution.generate_puzzle(difficulty.num_holes());

        Self {
            difficulty,
            start: Some(Instant::now()),
            grid: puzzle.grid.map(|row| row.map(Cell::new)),
            solution: solution.grid.map(|row| row.map(Cell::new)),
            ..Default::default()
        }
    }

    pub fn load(bytes: &[u8]) -> Result<Self> {
        let save: Save = bincode::deserialize(bytes)?;
        Ok(Self {
            hints: save.hints,
            checks: save.checks,
            difficulty: save.difficulty,
            start: Some(Instant::now()),
            elapsed: Duration::from_secs(save.elapsed),
            grid: save.grid,
            solution: save.solution,
            ..Default::default()
        })
    }

    pub fn save(&self) -> Result<Vec<u8>> {
        let save = Save {
            grid: self.grid,
            solution: self.solution,
            difficulty: self.difficulty,
            elapsed: self.elapsed().as_secs(),
            checks: self.checks,
            hints: self.hints,
        };
        bincode::serialize(&save).map_err(Into::into)
    }

    pub fn grid(&self) -> &[[Cell; SIZE]; SIZE] {
        &self.grid
    }

    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    pub fn hints(&self) -> usize {
        self.hints
    }

    pub fn checks(&self) -> usize {
        self.checks
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.state, GameState::Paused)
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, GameState::Running)
    }

    pub fn elapsed(&self) -> Duration {
        match self.state {
            GameState::Running => self.elapsed + self.start.unwrap().elapsed(),
            GameState::Paused => self.elapsed,
            _ => self.elapsed,
        }
    }

    pub fn at(&self, x: usize, y: usize) -> Cell {
        self.grid[y][x]
    }

    pub fn is_locked(&self, x: usize, y: usize) -> bool {
        self.at(x, y).locked
    }

    fn can_check(&self) -> bool {
        self.is_running() && self.checks < MAX_CHECKS
    }

    fn can_hint(&self) -> bool {
        self.is_running() && self.hints < MAX_HINTS
    }

    fn solve(&mut self) {
        self.elapsed = self.elapsed();
        self.state = GameState::Solved;
    }

    pub fn complete(&mut self) {
        if !self.is_running() {
            return;
        }
        self.solve();
        for y in 0..SIZE {
            for x in 0..SIZE {
                if !self.grid[y][x].locked {
                    let is_correct = self.grid[y][x].value == self.solution[y][x].value;
                    self.grid[y][x].value = self.solution[y][x].value;
                    self.grid[y][x].checked = Some(is_correct);
                }
            }
        }
    }

    pub fn check(&mut self) {
        if !self.can_check() {
            return;
        }

        let mut checked = false;
        for i in 0..SIZE {
            for j in 0..SIZE {
                let cell = &mut self.grid[i][j];
                if cell.locked || cell.value == 0 || cell.checked.unwrap_or(false) {
                    continue;
                }

                cell.checked = Some(cell.value == self.solution[i][j].value);
                checked = true;
            }
        }

        if checked {
            self.checks += 1;
        }
    }

    pub fn hint(&mut self) {
        if !self.can_hint() {
            return;
        }

        let mut rng = rand::thread_rng();
        let mut positions: Vec<(usize, usize)> = (0..SIZE)
            .flat_map(|r| (0..SIZE).map(move |c| (r, c)))
            .collect();
        positions.shuffle(&mut rng);

        for (y, x) in positions {
            let cell = &mut self.grid[y][x];
            if cell.locked || cell.value != 0 {
                continue;
            }

            cell.value = self.solution[y][x].value;
            cell.checked = Some(true);
            self.hints += 1;
            break;
        }
    }

    pub fn pause(&mut self) {
        if self.is_running() {
            self.toggle_pause();
        }
    }

    pub fn toggle_pause(&mut self) {
        self.state = match self.state {
            GameState::Paused => {
                self.start = Some(Instant::now());
                GameState::Running
            }
            GameState::Running => {
                self.elapsed += self.start.unwrap().elapsed();
                self.start = None;
                GameState::Paused
            }
            _ => self.state,
        };
    }

    pub fn undo_last_move(&mut self) -> Option<(usize, usize)> {
        if !self.is_running() {
            return None;
        }
        let mv = self.movements.pop()?;
        self.grid[mv.y][mv.x].value = mv.old;
        Some((mv.x, mv.y))
    }

    pub fn update_cell(&mut self, x: usize, y: usize, value: u8) {
        if !self.is_running() || self.is_locked(x, y) {
            return;
        }
        let old = std::mem::replace(&mut self.grid[y][x].value, value);
        self.movements.push(Move { x, y, old });
        self.grid[y][x].checked = None;

        if self.is_solved() {
            self.elapsed = self.elapsed();
            self.state = GameState::Won;
        }
    }

    pub fn clear_board(&mut self) {
        if !self.is_running() {
            return;
        }
        for row in self.grid.iter_mut() {
            for cell in row.iter_mut() {
                if !cell.locked {
                    cell.value = 0;
                }
            }
        }
    }

    fn is_solved(&self) -> bool {
        self.grid.iter().enumerate().all(|(y, row)| {
            row.iter()
                .enumerate()
                .all(|(x, cell)| cell.value == self.solution[y][x].value)
        })
    }
}

impl Board {
    pub fn generate() -> Self {
        let mut board = Self::default();
        board.fill_diagonals();
        board.fill_remaining(0, 0);
        board
    }

    fn generate_puzzle(&self, num_holes: usize) -> Board {
        let mut rng = rand::thread_rng();
        let mut positions: Vec<(usize, usize)> = (0..SIZE)
            .flat_map(|r| (0..SIZE).map(move |c| (r, c)))
            .collect();
        positions.shuffle(&mut rng);

        let mut puzzle = self.clone();
        for &(row, col) in positions.iter().take(num_holes) {
            let backup = self.grid[row][col];
            puzzle.grid[row][col] = 0;

            let mut test_board = puzzle.clone();
            if test_board.count_solutions(2) != 1 {
                puzzle.grid[row][col] = backup;
            }
        }
        puzzle
    }

    fn is_valid(&self, row: usize, col: usize, value: u8) -> bool {
        // Check row and column
        for i in 0..SIZE {
            if self.grid[row][i] == value || self.grid[i][col] == value {
                return false;
            }
        }
        // Check 3x3 subgrid
        let start_row = row / SUBGRID_SIZE * SUBGRID_SIZE;
        let start_col = col / SUBGRID_SIZE * SUBGRID_SIZE;
        for i in 0..SUBGRID_SIZE {
            for j in 0..SUBGRID_SIZE {
                if self.grid[start_row + i][start_col + j] == value {
                    return false;
                }
            }
        }
        true
    }

    fn fill_diagonals(&mut self) {
        for i in 0..SIZE {
            if i % SUBGRID_SIZE == 0 {
                self.fill_box(i, i);
            }
        }
    }

    fn fill_box(&mut self, row: usize, col: usize) {
        let mut rng = rand::thread_rng();
        let mut numbers: Vec<u8> = (1..=9).collect();
        numbers.shuffle(&mut rng);

        for i in 0..SUBGRID_SIZE {
            for j in 0..SUBGRID_SIZE {
                self.grid[row + i][col + j] = numbers.pop().unwrap();
            }
        }
    }

    fn fill_remaining(&mut self, row: usize, col: usize) -> bool {
        let mut rng = rand::thread_rng();
        let mut numbers: Vec<u8> = (1..=9).collect();
        numbers.shuffle(&mut rng);

        // Board filled
        if row == SIZE - 1 && col == SIZE {
            return true;
        }

        // Move to next row
        if col == SIZE {
            return self.fill_remaining(row + 1, 0);
        }

        // Skip filled cells
        if self.grid[row][col] != 0 {
            return self.fill_remaining(row, col + 1);
        }

        for &num in &numbers {
            if self.is_valid(row, col, num) {
                self.grid[row][col] = num;
                // Move to next column
                if self.fill_remaining(row, col + 1) {
                    return true;
                }
                self.grid[row][col] = 0;
            }
        }
        false
    }

    fn count_solutions(&mut self, limit: usize) -> usize {
        let mut count = 0;
        self.solve_with_limit(&mut count, limit);
        count
    }

    fn solve_with_limit(&mut self, count: &mut usize, limit: usize) -> bool {
        for row in 0..SIZE {
            for col in 0..SIZE {
                if self.grid[row][col] == 0 {
                    for num in 1..=9 {
                        if self.is_valid(row, col, num) {
                            self.grid[row][col] = num;
                            if self.solve_with_limit(count, limit) {
                                return true;
                            }
                            self.grid[row][col] = 0;
                        }
                    }
                    return false;
                }
            }
        }
        *count += 1;
        *count >= limit
    }
}
