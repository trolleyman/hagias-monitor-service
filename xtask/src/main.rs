use anyhow::Result;

pub mod cli;
pub mod command;
pub mod fs;
pub mod ignore;
pub mod print;
pub mod watch;

fn main() -> Result<()> {
    std::process::exit(cli::run()?)
}
