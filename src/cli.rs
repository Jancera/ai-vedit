use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::subtitles::SubtitlePosition;

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

    /// Minimum duration (seconds) each beat should have; shorter beats are merged with neighbors
    #[arg(long, default_value = "5.0")]
    pub min_beat_duration: f64,

    /// Generate burned-in subtitles from the transcript and embed them in plan.json
    #[arg(long)]
    pub subtitles: bool,

    /// Font to use for subtitles
    #[arg(long)]
    pub subtitle_font: Option<String>,

    /// Font size for subtitles
    #[arg(long)]
    pub subtitle_font_size: Option<u32>,

    /// Primary color for subtitles (hex #RRGGBB)
    #[arg(long)]
    pub subtitle_primary_color: Option<String>,

    /// Make subtitles bold
    #[arg(long)]
    pub subtitle_bold: bool,

    /// Make subtitles italic
    #[arg(long)]
    pub subtitle_italic: bool,

    /// Make subtitles uppercase
    #[arg(long)]
    pub subtitle_uppercase: bool,

    /// Position of subtitles on screen
    #[arg(long)]
    pub subtitle_position: Option<SubtitlePosition>,

    /// Vertical margin for subtitles
    #[arg(long)]
    pub subtitle_margin_vertical: Option<u32>,

    /// Maximum characters per line for subtitles
    #[arg(long)]
    pub subtitle_max_chars_per_line: Option<u32>,

    /// Maximum lines per cue for subtitles
    #[arg(long)]
    pub subtitle_max_lines: Option<u32>,

    /// Maximum duration (seconds) per cue for subtitles
    #[arg(long)]
    pub subtitle_max_duration: Option<f64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum AspectRatio {
    #[value(name = "16:9")]
    Sixteen9,
    #[value(name = "9:16")]
    Nine16,
}
