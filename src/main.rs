mod assets;
mod cache;
mod cli;
mod config;
mod library;
mod plan_file;
mod planner;
mod render;
mod whisper;

use clap::Parser;
use cli::{Cli, Commands, PlanArgs, RenderArgs};
use config::Config;
use plan_file::PlanFile;

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

    let base_url = match std::env::var("AI_VEDIT_OPENAI_BASE_URL") {
        Ok(url) => {
            eprintln!("note: using OpenAI base URL {url} from AI_VEDIT_OPENAI_BASE_URL");
            url
        }
        Err(_) => "https://api.openai.com".to_string(),
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

    let existing_categories = match library::discover_categories(&args.assets) {
        Ok(categories) => categories,
        Err(e) => {
            eprintln!("error: cannot read asset library {:?}: {e}", args.assets);
            std::process::exit(1);
        }
    };

    let plan = match planner::plan_beats(
        &base_url,
        &config.openai_api_key,
        &transcript,
        &existing_categories,
    ) {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("error: planning failed: {e}");
            std::process::exit(1);
        }
    };

    let plan_file = PlanFile {
        audio_path: args.audio.clone(),
        aspect: args.aspect,
        beats: plan.beats,
    };

    let plan_path = std::path::Path::new("plan.json");
    if let Err(e) = plan_file::save(plan_path, &plan_file) {
        eprintln!("error: failed to write plan file: {e}");
        std::process::exit(1);
    }

    print_report(&plan_file, plan_path, &args.assets);
}

fn print_report(plan_file: &PlanFile, plan_path: &std::path::Path, assets_dir: &std::path::Path) {
    use std::collections::BTreeMap;

    let mut budgets: BTreeMap<&str, (usize, f64)> = BTreeMap::new();
    for beat in &plan_file.beats {
        let entry = budgets.entry(beat.category.as_str()).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += beat.end - beat.start;
    }

    println!("plan: wrote {:?}", plan_path);
    println!("assets library: {:?}", assets_dir);
    println!("time budget by category:");
    for (category, (count, seconds)) in &budgets {
        let beat_word = if *count == 1 { "beat" } else { "beats" };
        println!("  {category}: {count} {beat_word}, {seconds:.1}s");
    }

    let new_categories: Vec<&str> = plan_file
        .beats
        .iter()
        .filter(|b| b.is_new_category)
        .map(|b| b.category.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    if !new_categories.is_empty() {
        println!("categories to create before rendering:");
        for category in new_categories {
            println!("  {category}");
        }
    }
}

fn run_render(args: RenderArgs) {
    println!(
        "render: not yet implemented (plan={:?}, assets={:?}, out={:?}, aspect={:?})",
        args.plan, args.assets, args.out, args.aspect
    );
}
