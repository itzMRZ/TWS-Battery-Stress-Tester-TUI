use anyhow::{Context, Result};
use tws_tester::cli;
use tws_tester::ui::App;

#[tokio::main]
async fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        None | Some("tui") => run_tui().await,
        Some("probe") => {
            tws_tester::probe::run().await?;
            Ok(())
        }
        Some("--history" | "history") => cli::open_history(),
        Some("--update" | "update") => cli::update(),
        Some("-V" | "--version" | "version") => {
            cli::print_version();
            Ok(())
        }
        Some("-h" | "--help" | "help") => {
            print!("{}", cli::HELP);
            Ok(())
        }
        Some(other) => anyhow::bail!("unknown command {other}\n\n{}", cli::HELP),
    }
}

async fn run_tui() -> Result<()> {
    install_panic_restore();
    let mut app = App::new().await?;
    let mut terminal = ratatui::try_init().context("terminal (need a TTY)")?;
    let result = app.run(&mut terminal).await;
    ratatui::restore();
    result
}

fn install_panic_restore() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        hook(info);
    }));
}
