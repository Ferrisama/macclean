mod ask;
mod cleaners;
mod cli;
mod core;
mod tui;
mod ui;

fn main() -> anyhow::Result<()> {
    cli::run()
}
