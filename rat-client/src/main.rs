use color_eyre::Result;

mod app;
mod client;
mod tui;
mod ui;

use app::App;

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = tui::init()?;
    let app_result = App { ..App::default() }.run(&mut terminal).await;

    if let Err(err) = tui::restore() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {err}");
    }
    app_result
}

/// WASM entry point — Trunk calls this as the binary entry; sets up DomBackend.
#[cfg(target_arch = "wasm32")]
fn main() {
    App::start_wasm();
}
