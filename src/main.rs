mod ask;
mod cleaners;
mod cli;
mod core;
mod doctor;
mod tui;
mod ui;

fn main() -> anyhow::Result<()> {
    cli::run()
}
