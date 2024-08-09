use anyhow::Result;
use ratatui::{
    crossterm::event::{poll, read, Event, KeyCode, KeyEvent, KeyEventKind},
    prelude::*,
    style::palette::tailwind::SLATE,
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
    Terminal,
};

use crate::sudoku::{Cell, Difficulty, Sudoku, MAX_CHECKS, MAX_HINTS};

const SAVE_FILE: &str = "sudoku.save";

#[derive(Default)]
pub struct App {
    main_menu: MenuWidget,
    new_game_menu: MenuWidget,
    game: GameWidget,
    current_screen: Screen,
    quit: bool,
}

#[derive(Default)]
pub enum Screen {
    #[default]
    MainMenu,
    NewGameMenu,
    Game,
}

pub struct GameWidget {
    game: Sudoku,
    cursor: (usize, usize),
    show_controls: bool,
}

#[derive(Copy, Clone)]
enum Action {
    MoveCursor(isize, isize),
    UpdateCell(u8),
    ClearCell,
    Undo,
    ClearBoard,
    NewGame(Option<Difficulty>),
    SaveGame,
    LoadGame,
    Pause,
    TogglePause,
    Hint,
    Solve,
    Check,
    ToggleControls,
    Quit,
}

impl Widget for &GameWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [main, controls] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(13), Constraint::Length(3)])
            .flex(layout::Flex::Center)
            .areas(area);

        let [sidebar, game] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(16), Constraint::Length(42)])
            .flex(layout::Flex::Center)
            .areas(main);
        self.board().render(game, buf);

        let [timer, diff, hints, checks] = Layout::default()
            .direction(Direction::Vertical)
            .constraints(Constraint::from_lengths([3, 3, 3, 3]))
            .areas(sidebar);
        self.timer().render(timer, buf);
        self.difficulty().render(diff, buf);
        self.hints().render(hints, buf);
        self.checks().render(checks, buf);

        if self.show_controls {
            let [controls] = Layout::default()
                .direction(Direction::Horizontal)
                .flex(layout::Flex::Center)
                .constraints([Constraint::Fill(1)])
                .areas(controls);

            self.controls().render(controls, buf);
        }

        if self.game.is_paused() {
            let area = centered_rect(90, 30, game);
            Clear.render(area, buf);
            self.pause_popup().render(area, buf);
        }
    }
}

impl Default for GameWidget {
    fn default() -> Self {
        Self {
            game: Sudoku::generate(Difficulty::Hard),
            cursor: (0, 0),
            show_controls: true,
        }
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.current_screen {
            Screen::MainMenu => self.main_menu.render(area, buf),
            Screen::Game => self.game.render(area, buf),
            Screen::NewGameMenu => self.new_game_menu.render(area, buf),
        }
    }
}

impl App {
    pub fn new() -> Self {
        let options = vec![
            ("New Game", Action::NewGame(None)),
            ("Load Game", Action::LoadGame),
            ("Quit", Action::Quit),
        ];
        let main_menu = MenuWidget::new(options);

        let options = vec![
            ("Easy", Action::NewGame(Some(Difficulty::Easy))),
            ("Medium", Action::NewGame(Some(Difficulty::Medium))),
            ("Hard", Action::NewGame(Some(Difficulty::Hard))),
            ("Expert", Action::NewGame(Some(Difficulty::Expert))),
            ("< Back", Action::Quit),
        ];
        let new_game_menu = MenuWidget::new(options);

        Self {
            main_menu,
            new_game_menu,
            ..Default::default()
        }
    }

    pub fn run(&mut self, mut term: Terminal<impl Backend>) -> Result<()> {
        while self.is_running() {
            self.draw_current_screen(&mut term)?;
            let mut current_message = self.handle_events()?;

            while let Some(message) = current_message {
                current_message = self.update(message);
            }
        }
        Ok(())
    }

    fn is_running(&self) -> bool {
        !self.quit
    }

    fn draw_current_screen(&self, term: &mut Terminal<impl Backend>) -> Result<()> {
        term.draw(|f| match self.current_screen {
            Screen::MainMenu => f.render_widget(&self.main_menu, f.size()),
            Screen::NewGameMenu => f.render_widget(&self.new_game_menu, f.size()),
            Screen::Game => f.render_widget(&self.game, f.size()),
        })?;
        Ok(())
    }

    fn handle_quit(&mut self) -> Option<Action> {
        match self.current_screen {
            Screen::Game => self.current_screen = Screen::MainMenu,
            Screen::NewGameMenu => self.current_screen = Screen::MainMenu,
            Screen::MainMenu => self.quit = true,
        }
        None
    }

    fn update_current_screen(&mut self, message: Action) -> Option<Action> {
        match self.current_screen {
            Screen::Game => self.game.update(message),
            Screen::MainMenu => {
                match message {
                    Action::NewGame(_) => self.current_screen = Screen::NewGameMenu,
                    Action::LoadGame => {
                        self.current_screen = Screen::Game;
                        self.game.load_game()
                    }
                    _ => (),
                }
                None
            }
            Screen::NewGameMenu => {
                if let Action::NewGame(d) = message {
                    self.current_screen = Screen::Game;
                    self.game.new_game(d.unwrap_or_default());
                }
                None
            }
        }
    }

    fn update(&mut self, message: Action) -> Option<Action> {
        match message {
            Action::Quit => self.handle_quit(),
            _ => self.update_current_screen(message),
        }
    }

    fn handle_events(&mut self) -> Result<Option<Action>> {
        if poll(std::time::Duration::from_millis(100))? {
            return match self.current_screen {
                Screen::MainMenu => self.main_menu.handle_events(),
                Screen::Game => self.game.handle_events(),
                Screen::NewGameMenu => self.new_game_menu.handle_events(),
            };
        }
        Ok(None)
    }
}

impl GameWidget {
    fn move_cursor(&mut self, dx: isize, dy: isize) {
        if !self.game.is_running() {
            return;
        }
        let (x, y) = self.cursor;
        let x = dx.saturating_add(x as isize).clamp(0, 8) as usize;
        let y = dy.saturating_add(y as isize).clamp(0, 8) as usize;
        self.cursor = (x, y);
    }

    fn handle_update_cell(&mut self, value: u8) {
        let (x, y) = self.cursor;
        self.game.update_cell(x, y, value);
    }

    fn handle_undo(&mut self) {
        self.cursor = self.game.undo_last_move().unwrap_or(self.cursor);
    }

    fn new_game(&mut self, difficulty: Difficulty) {
        self.game = Sudoku::generate(difficulty);
        self.cursor = (0, 0);
    }

    fn save_game(&mut self) {
        let bytes = self.game.save().unwrap();
        std::fs::write(SAVE_FILE, bytes).unwrap();
    }

    fn load_game(&mut self) {
        let bytes = std::fs::read(SAVE_FILE).unwrap();
        self.game = Sudoku::load(&bytes).unwrap();
    }

    fn update(&mut self, message: Action) -> Option<Action> {
        match message {
            Action::MoveCursor(dx, dy) => self.move_cursor(dx, dy),
            Action::UpdateCell(v) => self.handle_update_cell(v),
            Action::ClearCell => self.handle_update_cell(0),
            Action::Undo => self.handle_undo(),
            Action::ClearBoard => self.game.clear_board(),
            Action::TogglePause => self.game.toggle_pause(),
            Action::Pause => self.game.pause(),
            Action::SaveGame => self.save_game(),
            Action::LoadGame => self.load_game(),
            Action::NewGame(_) => self.new_game(self.game.difficulty()),
            Action::Hint => self.game.hint(),
            Action::Solve => self.game.complete(),
            Action::Check => self.game.check(),
            Action::ToggleControls => self.show_controls = !self.show_controls,
            _ => {}
        }
        None
    }

    fn handle_events(&mut self) -> Result<Option<Action>> {
        if poll(std::time::Duration::from_millis(100))? {
            let msg = match read()? {
                Event::Key(e) if e.kind == KeyEventKind::Press => self.handle_key_event(e),
                Event::FocusLost => Some(Action::Pause),
                _ => None,
            };
            return Ok(msg);
        }
        Ok(None)
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Option<Action> {
        let msg = match event.code {
            KeyCode::Char('?') => Action::ToggleControls,
            /* Shift modifier */
            KeyCode::Char('C') => Action::Solve,
            KeyCode::Char('N') => Action::NewGame(Some(self.game.difficulty())),
            KeyCode::Char('B') => Action::ClearBoard,
            KeyCode::Char('S') => Action::SaveGame,
            KeyCode::Char('Q') | KeyCode::Esc => Action::Quit,
            /* */
            KeyCode::Char('p') => Action::TogglePause,
            KeyCode::Char('c') => Action::Check,
            KeyCode::Char('t') => Action::Hint,
            KeyCode::Char('u') => Action::Undo,
            KeyCode::Char('x') => Action::ClearCell,
            KeyCode::Char('h') | KeyCode::Left => Action::MoveCursor(-1, 0),
            KeyCode::Char('l') | KeyCode::Right => Action::MoveCursor(1, 0),
            KeyCode::Char('k') | KeyCode::Up => Action::MoveCursor(0, -1),
            KeyCode::Char('j') | KeyCode::Down => Action::MoveCursor(0, 1),
            KeyCode::Char(c @ '1'..='9') => {
                let value = c.to_digit(10).unwrap() as u8;
                Action::UpdateCell(value)
            }
            _ => return None,
        };
        Some(msg)
    }
}

impl GameWidget {
    const TEXT_COLOR: Color = SLATE.c400;

    fn controls(&self) -> impl Widget {
        let keys = [
            ("←↑→↓", "Move"),
            ("0-9", "Update"),
            ("u", "Undo"),
            ("x", "Clear cell"),
            ("p", "Pause"),
            ("t", "Hint"),
            ("c", "Check"),
            ("^C", "Solve"),
            ("^N", "New game"),
            ("^B", "Clear board"),
            ("^S", "Save game"),
            ("?", "Show/hide controls"),
            ("^Q", "Quit"),
        ];

        let kstyle = Style::default().fg(Color::White).bg(Color::DarkGray);
        let dstyle = Style::default().fg(Self::TEXT_COLOR).bg(Color::Black);

        let line: Line = keys
            .iter()
            .flat_map(|(key, desc)| {
                let key = Span::styled(format!(" {key} "), kstyle);
                let desc = Span::styled(format!(" {desc} "), dstyle);
                [key, desc]
            })
            .collect();

        Paragraph::new(line)
            .wrap(Wrap { trim: true })
            .left_aligned()
    }

    fn pause_popup(&self) -> impl Widget {
        let text = vec![Line::from("press 'P' to resume").fg(Self::TEXT_COLOR)];
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Paused")
                    .fg(Self::TEXT_COLOR)
                    .title_alignment(Alignment::Center),
            )
            .centered()
    }

    fn timer(&self) -> impl Widget {
        let text = {
            let secs = self.game.elapsed().as_secs();
            let mins = secs / 60;
            let secs = secs % 60;
            format!("{:02}:{:02}", mins, secs)
        };
        Paragraph::new(text).centered().fg(Self::TEXT_COLOR).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Time")
                .title_alignment(Alignment::Center),
        )
    }

    fn difficulty(&self) -> impl Widget {
        Paragraph::new(self.game.difficulty().as_str())
            .centered()
            .fg(Self::TEXT_COLOR)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Difficulty")
                    .title_alignment(Alignment::Center),
            )
    }

    fn hints(&self) -> impl Widget {
        let text = format!("{}/{}", self.game.hints(), MAX_HINTS);
        Paragraph::new(text).centered().fg(Self::TEXT_COLOR).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Hints")
                .title_alignment(Alignment::Center),
        )
    }

    fn checks(&self) -> impl Widget {
        let text = format!("{}/{}", self.game.checks(), MAX_CHECKS);
        Paragraph::new(text).centered().fg(Self::TEXT_COLOR).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Checks")
                .title_alignment(Alignment::Center),
        )
    }

    fn cell_style(&self, cell: Cell, x: usize, y: usize) -> Style {
        if self.game.is_paused() {
            return Style::default();
        }
        let (cx, cy) = self.cursor;
        let at_cursor = self.game.at(cx, cy).value;

        let highlight_value =
            |value| self.game.is_running() && value == at_cursor && at_cursor != 0;

        let fg_color = match cell {
            Cell { value: 0, .. } => Color::DarkGray,
            Cell { value, .. } if highlight_value(value) => Color::LightYellow,
            Cell { locked: true, .. } => Color::White,
            Cell {
                checked: Some(correct),
                ..
            } => {
                if correct {
                    Color::Green
                } else {
                    Color::Red
                }
            }
            _ => Color::Blue,
        };

        let bg_color = match (x == cx, y == cy) {
            _ if !self.game.is_running() => Color::Reset,
            (true, true) => Color::DarkGray,
            (true, false) | (false, true) => Color::Black,
            _ => {
                let (cx, cy) = (cx / 3, cy / 3);
                if x / 3 == cx && y / 3 == cy {
                    Color::Black
                } else {
                    Color::Reset
                }
            }
        };

        Style::default().fg(fg_color).bg(bg_color)
    }

    fn board(&self) -> impl Widget {
        let mut content = Text::default();
        for (y, row) in self.game.grid().iter().enumerate() {
            if y % 3 == 0 && y != 0 {
                content.push_line("\n");
            }
            for (x, &cell) in row.iter().enumerate() {
                if x % 3 == 0 && x != 0 {
                    content.push_span(("│").fg(Self::TEXT_COLOR));
                }
                let num = cell.value;
                let ch = if num == 0 { b'.' } else { num + b'0' } as char;
                let ch = format!("{:^3} ", if self.game.is_paused() { '*' } else { ch });
                let style = self.cell_style(cell, x, y);
                content.push_span(Span::styled(ch, style));
            }
            content.push_line("\n");
        }

        Paragraph::new(content)
            .centered()
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Self::TEXT_COLOR))
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

#[derive(Default)]
struct MenuWidget {
    options: Vec<(String, Action)>,
    selected: usize,
}

impl Widget for &MenuWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let centered = centered_rect(30, 30, area);
        let text = self
            .options
            .iter()
            .enumerate()
            .map(|(i, (option, _))| {
                let style = if i == self.selected {
                    Style::default().fg(SLATE.c400).bg(SLATE.c800)
                } else {
                    Style::default().fg(SLATE.c400)
                };
                Line::styled(option, style).centered()
            })
            .collect::<Vec<_>>();

        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Sudoku")
                    .title_alignment(Alignment::Center)
                    .fg(SLATE.c400),
            )
            .render(centered, buf);
    }
}

impl MenuWidget {
    fn new<T>(options: Vec<(T, Action)>) -> Self
    where
        T: Into<String>,
    {
        Self {
            options: options.into_iter().map(|(s, m)| (s.into(), m)).collect(),
            selected: 0,
        }
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Option<Action> {
        match event.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.selected = (self.selected + 1) % self.options.len();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1) % self.options.len();
            }
            KeyCode::Enter => {
                let (_, msg) = &self.options[self.selected];
                return Some(*msg);
            }
            _ => {}
        }
        None
    }

    fn handle_events(&mut self) -> Result<Option<Action>> {
        if poll(std::time::Duration::from_millis(100))? {
            let msg = match read()? {
                Event::Key(e) if e.kind == KeyEventKind::Press => self.handle_key_event(e),
                _ => None,
            };
            return Ok(msg);
        }
        Ok(None)
    }
}
