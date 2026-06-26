mod core;
mod source_path;
mod lang;
mod crap;
mod mutate;
mod runner;
mod skip;
mod scheduler;
mod report;
mod config;
mod doctor;
mod cli;
mod app;

fn main() -> anyhow::Result<()> {
    app::run()
}
