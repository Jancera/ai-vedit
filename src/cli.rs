use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "ai-vedit",
    version,
    about = "Turn a narrated audio script into an edited video"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Transcribe the audio and produce a shot list / asset plan
    Plan(PlanArgs),
    /// Render the final video from a plan and asset library
    Render(RenderArgs),
}

#[derive(Args, Debug)]
pub struct PlanArgs {
    /// Path to the narration audio file (mp3)
    #[arg(long)]
    pub audio: PathBuf,

    /// Path to the asset library directory
    #[arg(long, default_value = "assets")]
    pub assets: PathBuf,

    /// Output aspect ratio
    #[arg(long, value_enum, default_value = "16:9")]
    pub aspect: AspectRatio,
}

#[derive(Args, Debug)]
pub struct RenderArgs {
    /// Path to the plan file produced by `plan`
    #[arg(long)]
    pub plan: PathBuf,

    /// Path to the asset library directory
    #[arg(long, default_value = "assets")]
    pub assets: PathBuf,

    /// Path to write the rendered video to
    #[arg(long, default_value = "output.mp4")]
    pub out: PathBuf,

    /// Output aspect ratio
    #[arg(long, value_enum, default_value = "16:9")]
    pub aspect: AspectRatio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AspectRatio {
    #[value(name = "16:9")]
    Sixteen9,
    #[value(name = "9:16")]
    Nine16,
}
