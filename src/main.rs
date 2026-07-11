mod cli;
mod core;
mod ui;
mod cleaners;
mod tui;

fn main() -> anyhow::Result<()> {
    cli::run()
}
