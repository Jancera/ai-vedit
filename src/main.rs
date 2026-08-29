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
    if let Err(e) = std::fs::File::open(&args.audio) {
        eprintln!("error: cannot open audio file {:?}: {e}", args.audio);
        std::process::exit(1);
    }

    if args.min_beat_duration <= 0.0 {
        eprintln!("error: --min-beat-duration must be greater than 0");
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
            eprintln!(
                "error: cannot hash audio file {:?} for caching: {e}",
                args.audio
            );
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

    if let Err(e) = library::ensure_assets_dir(&args.assets) {
        eprintln!(
            "warning: could not create assets directory {:?}: {e}",
            args.assets
        );
    }

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
        args.min_beat_duration,
    ) {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("error: planning failed: {e}");
            std::process::exit(1);
        }
    };

    let plan_categories: Vec<&str> = plan.beats.iter().map(|b| b.category.as_str()).collect();
    let created_categories = match library::ensure_category_dirs(&args.assets, plan_categories) {
        Ok(created) => created,
        Err(e) => {
            eprintln!(
                "warning: could not create category directories under {:?}: {e}",
                args.assets
            );
            Vec::new()
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

    print_report(&plan_file, plan_path, &args.assets, &created_categories);
}

fn print_report(
    plan_file: &PlanFile,
    plan_path: &std::path::Path,
    assets_dir: &std::path::Path,
    created_categories: &[String],
) {
    use std::collections::BTreeMap;

    let mut budgets: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for beat in &plan_file.beats {
        let entry = budgets
            .entry(assets::normalize_category(&beat.category))
            .or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += beat.duration;
    }

    println!("plan: wrote {:?}", plan_path);
    println!("assets library: {:?}", assets_dir);
    println!("time budget by category:");
    for (category, (count, seconds)) in &budgets {
        let beat_word = if *count == 1 { "beat" } else { "beats" };
        println!("  {category}: {count} {beat_word}, {seconds:.1}s");
    }

    if !created_categories.is_empty() {
        println!("created category directories (add assets before rendering):");
        for category in created_categories {
            println!("  {category}");
        }
    }
}

fn run_render(args: RenderArgs) {
    let plan_file = match plan_file::load(&args.plan) {
        Ok(pf) => pf,
        Err(e) => {
            eprintln!("error: failed to load plan file {:?}: {e}", args.plan);
            std::process::exit(1);
        }
    };

    let mut selector = match assets::AssetSelector::new(&args.assets) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read asset library {:?}: {e}", args.assets);
            std::process::exit(1);
        }
    };

    let tmp_dir = match std::env::current_dir() {
        Ok(cwd) => cwd.join(".render-tmp"),
        Err(e) => {
            eprintln!("error: cannot determine current directory: {e}");
            std::process::exit(1);
        }
    };
    let tmp_dir = tmp_dir.as_path();
    if let Err(e) = std::fs::create_dir_all(tmp_dir) {
        eprintln!("error: cannot create temp directory {tmp_dir:?}: {e}");
        std::process::exit(1);
    }

    if args.aspect != plan_file.aspect {
        eprintln!(
            "warning: --aspect {:?} differs from the plan's aspect {:?}; using the plan's aspect",
            args.aspect, plan_file.aspect
        );
    }
    let resolution = render::resolution_for(plan_file.aspect);
    let total = plan_file.beats.len();
    if plan_file.beats.is_empty() {
        eprintln!("error: plan has no beats to render");
        std::process::exit(1);
    }
    let mut clip_paths = Vec::new();

    for (i, beat) in plan_file.beats.iter().enumerate() {
        let beat_num = i + 1;
        let duration = beat.duration;
        if duration <= 0.0 {
            eprintln!("error: beat {beat_num} has a non-positive duration ({duration:.3}s)");
            std::process::exit(1);
        }
        println!(
            "rendering beat {beat_num}/{total} ({}, {duration:.1}s)...",
            beat.category
        );

        let selection = match selector.select(&beat.category) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        };

        if selection.used_fallback {
            eprintln!(
                "warning: beat {beat_num} ({}) has no assets, using general/",
                beat.category
            );
        }

        let clip_filename = format!("beat-{beat_num:03}.mp4");
        let clip_path = tmp_dir.join(&clip_filename);

        let ffmpeg_args = match selection.asset.kind {
            assets::AssetKind::Image => {
                render::ken_burns_command(&selection.asset.path, duration, resolution, &clip_path)
            }
            assets::AssetKind::Video => {
                render::video_clip_command(&selection.asset.path, duration, resolution, &clip_path)
            }
        };

        if let Err(e) = render::run_ffmpeg(&ffmpeg_args) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }

        clip_paths.push(std::path::PathBuf::from(&clip_filename));
    }

    println!("concatenating {total} clips...");
    let list_path = tmp_dir.join("concat-list.txt");
    let list_content = render::concat_list_content(&clip_paths);
    if let Err(e) = std::fs::write(&list_path, list_content) {
        eprintln!("error: cannot write concat list {list_path:?}: {e}");
        std::process::exit(1);
    }

    let concat_path = tmp_dir.join("concat.mp4");
    let concat_args = render::concat_command(&list_path, &concat_path);
    if let Err(e) = render::run_ffmpeg(&concat_args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }

    println!("overlaying narration audio...");
    let mux_args = render::mux_audio_command(&concat_path, &plan_file.audio_path, &args.out);
    if let Err(e) = render::run_ffmpeg(&mux_args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }

    if let Err(e) = std::fs::remove_dir_all(tmp_dir) {
        eprintln!("warning: could not remove temp directory {tmp_dir:?}: {e}");
    }

    println!("render: wrote {:?}", args.out);
}
