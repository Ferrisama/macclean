mod cli;
mod core;
mod ui;
mod cleaners;

fn main() -> anyhow::Result<()> {
    cli::run()
}
