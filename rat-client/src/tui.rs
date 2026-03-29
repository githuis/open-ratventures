use std::io;

// ─── Native (crossterm) ───────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
use std::io::Stdout;

#[cfg(not(target_arch = "wasm32"))]
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
};

#[cfg(not(target_arch = "wasm32"))]
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

#[cfg(not(target_arch = "wasm32"))]
pub fn init() -> io::Result<Tui> {
    use std::io::stdout;
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    set_panic_hook();
    Terminal::new(CrosstermBackend::new(stdout()))
}

#[cfg(not(target_arch = "wasm32"))]
fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        hook(panic_info);
    }));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn restore() -> io::Result<()> {
    use std::io::stdout;
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

// ─── WASM (ratzilla — placeholder until step 3) ───────────────────────────────

#[cfg(target_arch = "wasm32")]
use ratatui::{Terminal, backend::TestBackend};

/// Placeholder type — will be replaced with ratzilla's DomBackend in step 3.
#[cfg(target_arch = "wasm32")]
pub type Tui = Terminal<TestBackend>;

#[cfg(target_arch = "wasm32")]
pub fn init() -> io::Result<Tui> {
    Terminal::new(TestBackend::new(80, 24))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

#[cfg(target_arch = "wasm32")]
pub fn restore() -> io::Result<()> {
    Ok(())
}
