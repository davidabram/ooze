mod app;
mod cli;
mod config;
mod core;
mod crap;
mod doctor;
mod lang;
mod mutate;
mod report;
mod runner;
mod scheduler;
mod skip;
mod source_path;

fn main() -> anyhow::Result<()> {
    app::run()
}
