use anyhow::Result;
use rand::{prelude::SliceRandom, seq::IteratorRandom};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const SIZE: usize = 9;
const SUBGRID_SIZE: usize = 3;

pub const MAX_CHECKS: u8 = 3;
pub const MAX_HINTS: u8 = 3;

#[derive(Default)]
pub struct Sudoku {
    grid: [[Cell; SIZE]; SIZE],
    solution: [[u8; SIZE]; SIZE],
    state: GameState,
    movements: Vec<Move>,
    start: Option<Instant>,
    elapsed: Duration,
    difficulty: Difficulty,
    checks: u8,
    hints: u8,
}

#[derive(Serialize, Deserialize)]
struct Save {
    grid: [[Cell; SIZE]; SIZE],
    solution: [[u8; SIZE]; SIZE],
    difficulty: Difficulty,
    elapsed: u64,
    checks: u8,
    hints: u8,
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
    flags: u8,
}

const CELL_CHECKED: u8 = 0b0001;
const CELL_CORRECT: u8 = 0b0010;
const CELL_WRITABLE: u8 = 0b0100;

impl Cell {
    pub fn new(value: u8) -> Self {
        let flags = if value == 0 { CELL_WRITABLE } else { 0 };
        Self { value, flags }
    }

    pub fn uncheck(&mut self) {
        self.flags &= !CELL_CHECKED;
        self.flags &= !CELL_CORRECT;
    }

    pub fn check(&mut self, correct: bool) {
        self.flags |= CELL_CHECKED;
        if correct {
            self.flags |= CELL_CORRECT;
        }
    }

    pub fn writable(&self) -> bool {
        self.flags & CELL_WRITABLE != 0
    }

    pub fn checked(&self) -> bool {
        self.flags & CELL_CHECKED != 0
    }

    pub fn correct(&self) -> bool {
        self.flags & CELL_CORRECT != 0
    }
}

#[derive(Clone, Copy, Default)]
pub enum GameState {
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
            solution: solution.grid,
            ..Default::default()
        }
    }

    pub fn load(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes)
            .map(Self::from_save)
            .map_err(Into::into)
    }

    fn from_save(save: Save) -> Self {
        Self {
            hints: save.hints,
            checks: save.checks,
            difficulty: save.difficulty,
            start: Some(Instant::now()),
            elapsed: Duration::from_secs(save.elapsed),
            grid: save.grid,
            solution: save.solution,
            ..Default::default()
        }
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

    pub fn hints(&self) -> u8 {
        self.hints
    }

    pub fn checks(&self) -> u8 {
        self.checks
    }

    pub fn state(&self) -> GameState {
        self.state
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
            _ => self.elapsed,
        }
    }

    pub fn at(&self, x: usize, y: usize) -> Cell {
        self.grid[y][x]
    }

    pub fn writable(&self, x: usize, y: usize) -> bool {
        self.at(x, y).flags & CELL_WRITABLE != 0
    }

    fn can_check(&self) -> bool {
        self.is_running() && self.checks < MAX_CHECKS
    }

    fn can_hint(&self) -> bool {
        self.is_running() && self.hints < MAX_HINTS
    }

    pub fn complete(&mut self) {
        if !self.is_running() {
            return;
        }

        for y in 0..SIZE {
            for x in 0..SIZE {
                if !self.writable(x, y) {
                    continue;
                }
                let cell = &mut self.grid[y][x];
                let solution = self.solution[y][x];
                let is_correct = cell.value == 0 || cell.value == solution;
                self.grid[y][x].value = solution;
                self.grid[y][x].check(is_correct);
            }
        }

        self.elapsed = self.elapsed();
        self.state = GameState::Solved;
    }

    pub fn check(&mut self) {
        if !self.can_check() {
            return;
        }

        let mut checked = false;
        for i in 0..SIZE {
            for j in 0..SIZE {
                let cell = &mut self.grid[i][j];
                if !cell.writable() || cell.value == 0 || cell.checked() {
                    continue;
                }

                cell.check(cell.value == self.solution[i][j]);
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
            if !cell.writable() || cell.value != 0 {
                continue;
            }

            cell.value = self.solution[y][x];
            cell.check(true);
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
        if !self.is_running() || !self.writable(x, y) {
            return;
        }
        let old = std::mem::replace(&mut self.grid[y][x].value, value);
        self.movements.push(Move { x, y, old });
        self.grid[y][x].uncheck();

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
                if cell.writable() {
                    cell.value = 0;
                }
            }
        }
    }

    fn is_solved(&self) -> bool {
        self.grid.iter().enumerate().all(|(y, row)| {
            row.iter()
                .enumerate()
                .all(|(x, cell)| cell.value == self.solution[y][x])
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
