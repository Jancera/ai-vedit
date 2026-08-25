mod cache;
mod cli;
mod config;
mod whisper;

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
    if let Err(e) = std::fs::metadata(&args.audio) {
        eprintln!("error: cannot read audio file {:?}: {e}", args.audio);
        std::process::exit(1);
    }

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    let cache_path = match cache::cache_path_for(&args.audio) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("error: cannot read audio file {:?}: {e}", args.audio);
            std::process::exit(1);
        }
    };

    let transcript = match cache::load(&cache_path) {
        Some(cached) => cached,
        None => {
            let base_url = match std::env::var("AI_VEDIT_OPENAI_BASE_URL") {
                Ok(url) => {
                    eprintln!("note: using Whisper base URL {url} from AI_VEDIT_OPENAI_BASE_URL");
                    url
                }
                Err(_) => "https://api.openai.com".to_string(),
            };

            let transcript =
                match whisper::transcribe(&base_url, &config.openai_api_key, &args.audio) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("error: transcription failed: {e}");
                        std::process::exit(1);
                    }
                };

            if let Err(e) = cache::save(&cache_path, &transcript) {
                eprintln!("warning: could not cache transcript at {cache_path:?}: {e}");
            }

            transcript
        }
    };

    let total_duration = transcript.segments.last().map(|s| s.end).unwrap_or(0.0);
    let segment_word = if transcript.segments.len() == 1 {
        "segment"
    } else {
        "segments"
    };

    println!(
        "plan: transcribed {} {} ({:.1}s total, cache={:?}, assets={:?}, aspect={:?})",
        transcript.segments.len(),
        segment_word,
        total_duration,
        cache_path,
        args.assets,
        args.aspect
    );
}

fn run_render(args: RenderArgs) {
    println!(
        "render: not yet implemented (plan={:?}, assets={:?}, out={:?}, aspect={:?})",
        args.plan, args.assets, args.out, args.aspect
    );
}
