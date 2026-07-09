mod app;
mod cli;
mod config;
mod core;
mod crap;
mod doctor;
mod execution;
mod lang;
mod ledger;
mod mutate;
mod planning;
mod preset;
mod probe;
mod report;
mod scheduler;
mod skip;
mod source_path;
mod workspace;

fn main() -> anyhow::Result<()> {
    app::run()
}
