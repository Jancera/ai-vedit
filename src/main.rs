mod cli;
mod config;

use clap::Parser;
use cli::{Cli, Commands, PlanArgs, RenderArgs};
use config::Config;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Plan(args) => run_plan(args),
        Commands::Render(args) => run_render(args),
    }
}

fn run_plan(args: PlanArgs) {
    match Config::from_env() {
        Ok(_config) => {
            // Config::from_env() only returns Ok when OPENAI_API_KEY is set and non-empty,
            // so api_key_set is always true on this path.
            println!(
                "plan: not yet implemented (audio={:?}, assets={:?}, aspect={:?}, api_key_set=true)",
                args.audio, args.assets, args.aspect
            );
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

fn run_render(args: RenderArgs) {
    println!(
        "render: not yet implemented (plan={:?}, assets={:?}, out={:?}, aspect={:?})",
        args.plan, args.assets, args.out, args.aspect
    );
}
