mod action;
mod app;
mod clipboard;
mod command;
mod entry;
mod format;
mod id;
mod keybind;
mod store;
mod text;
mod ui;

use app::{App, Mode};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyboardEnhancementFlags,
    MouseButton, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};

fn main() -> io::Result<()> {
    install_panic_hook();
    let (mut terminal, kitty) = setup_terminal()?;
    let result = run(&mut terminal);
    teardown_terminal(&mut terminal, kitty)?;
    result
}

/// A panic unwinds straight past `teardown_terminal`, which would leave
/// the terminal in raw mode on the alternate screen with no cursor - i.e.
/// an unusable shell. Restore it first, then report the panic normally.
fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original(info);
    }));
}

/// Plain terminals have no distinguishable byte sequence for Ctrl+<digit>
/// (there's no ASCII control code for it), so `Ctrl+1..9` for jumping to
/// a history version only works on terminals implementing the Kitty
/// keyboard protocol (kitty, wezterm, foot, recent alacritty/tmux, …).
/// `h`/`l`/arrows/Tab always work everywhere and are the documented
/// primary way to navigate — see README.md.
fn setup_terminal() -> io::Result<(Terminal<CrosstermBackend<Stdout>>, bool)> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let kitty = supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
    }
    Ok((Terminal::new(CrosstermBackend::new(stdout))?, kitty))
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>, kitty: bool) -> io::Result<()> {
    disable_raw_mode()?;
    if kitty {
        execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags)?;
    }
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let action = keybind::resolve(app.mode_kind(), key, app.pending_key);
                action::apply(&mut app, action);
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                handle_mouse_click(&mut app, mouse.column, mouse.row);
            }
            _ => {}
        }
    }
    Ok(())
}

fn handle_mouse_click(app: &mut App, column: u16, row: u16) {
    if let Mode::History { .. } = &app.mode {
        if row == app.history_tab_row {
            if let Some((_, _, idx)) = app
                .history_tab_cols
                .iter()
                .find(|(start, end, _)| column >= *start && column < *end)
            {
                action::history_goto(app, *idx);
            }
        }
    }
}
