use anyhow::Result;

use app::App;

mod app;
mod sudoku;

fn main() -> Result<()> {
    tui::init_panic_hook()?;
    let terminal = tui::init_terminal()?;
    App::new().run(terminal)?;
    tui::restore_terminal()?;
    Ok(())
}

mod tui {
    use anyhow::Result;
    use ratatui::{
        crossterm::{
            event::{
                DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
            },
            execute,
            terminal::{
                disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
            },
        },
        prelude::*,
        Terminal,
    };
    use std::io::stdout;

    pub fn init_terminal() -> Result<Terminal<impl Backend>> {
        enable_raw_mode()?;
        execute!(
            stdout(),
            EnterAlternateScreen,
            EnableFocusChange,
            EnableMouseCapture
        )?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        Ok(terminal)
    }

    pub fn restore_terminal() -> Result<()> {
        disable_raw_mode()?;
        execute!(
            stdout(),
            LeaveAlternateScreen,
            DisableFocusChange,
            DisableMouseCapture
        )?;
        Ok(())
    }

    pub fn init_panic_hook() -> Result<()> {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = restore_terminal();
            default_hook(info);
        }));
        Ok(())
    }
}
