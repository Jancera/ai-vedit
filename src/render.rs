use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::{AspectRatio, FitMode};

pub const FPS: u32 = 60;
/// Factor by which the framed image is upscaled before `zoompan` in the
/// Ken Burns pass. `zoompan` quantizes its crop window to whole source
/// pixels every frame, so a slow zoom snaps by 1px intermittently and
/// shimmers; upscaling first shrinks that step by this factor, then
/// `zoompan`'s `s=` downscales back to the output. Costs a `4*width` x
/// `4*height` intermediate frame (7680x4320 for 1080p); images only.
const KEN_BURNS_UPSCALE: u32 = 4;

pub fn resolution_for(aspect: AspectRatio) -> (u32, u32) {
    match aspect {
        AspectRatio::Sixteen9 => (1920, 1080),
        AspectRatio::Nine16 => (1080, 1920),
    }
}
pub fn ken_burns_command(
    image_path: &Path,
    duration: f64,
    resolution: (u32, u32),
    fit: FitMode,
    output_path: &Path,
) -> Vec<String> {
    let (width, height) = resolution;
    let frames = (duration * FPS as f64).round() as u64;

    let mut vf = String::new();
    match fit {
        FitMode::Contain => {
            vf.push_str(&format!(
                "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,"
            ));
        }
        FitMode::Cover => {
            vf.push_str(&format!(
                "scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height},"
            ));
        }
    }

    let last_frame = frames.saturating_sub(1).max(1);
    let zoompan = format!(
        "scale=iw*{KEN_BURNS_UPSCALE}:ih*{KEN_BURNS_UPSCALE}:flags=lanczos,zoompan=z='1+0.15*on/{last_frame}':d={frames}:s={width}x{height}:fps={FPS}:x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)'"
    );
    vf.push_str(&zoompan);

    vec![
        "-hide_banner".to_string(),
        "-y".to_string(),
        "-loop".to_string(),
        "1".to_string(),
        "-i".to_string(),
        image_path.to_string_lossy().to_string(),
        "-t".to_string(),
        format!("{duration:.3}"),
        "-vf".to_string(),
        vf,
        "-r".to_string(),
        FPS.to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}

pub fn video_clip_command(
    video_path: &Path,
    duration: f64,
    resolution: (u32, u32),
    fit: FitMode,
    output_path: &Path,
) -> Vec<String> {
    let (width, height) = resolution;
    let vf = match fit {
        FitMode::Contain => format!(
            "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2"
        ),
        FitMode::Cover => format!(
            "scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height}"
        ),
    };

    vec![
        "-hide_banner".to_string(),
        "-y".to_string(),
        "-stream_loop".to_string(),
        "-1".to_string(),
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
        "-t".to_string(),
        format!("{duration:.3}"),
        "-vf".to_string(),
        vf,
        "-an".to_string(),
        "-r".to_string(),
        FPS.to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}



pub fn concat_list_content(clip_paths: &[PathBuf]) -> String {
    clip_paths
        .iter()
        .map(|p| format!("file '{}'\n", p.to_string_lossy()))
        .collect()
}

pub fn concat_command(list_path: &Path, output_path: &Path) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-y".to_string(),
        "-f".to_string(),
        "concat".to_string(),
        "-safe".to_string(),
        "0".to_string(),
        "-i".to_string(),
        list_path.to_string_lossy().to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}

pub fn mux_audio_command(video_path: &Path, audio_path: &Path, output_path: &Path) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
        "-i".to_string(),
        audio_path.to_string_lossy().to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-map".to_string(),
        "0:v".to_string(),
        "-map".to_string(),
        "1:a".to_string(),
        "-shortest".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}

/// Escapes a path so it can sit *inside single quotes* as the `filename`
/// of a `subtitles=` filter in an ffmpeg `-vf` filtergraph. Escapes, in
/// order, `\` -> `\\`, `'` -> `\'`, then `:` -> `\:`. Inside the quotes
/// `,` `;` `[` `]` are literal and need no escaping; `:` still must be
/// backslash-escaped because the filter's own option parser splits on it,
/// and `'`/`\` are the filtergraph quoting metacharacters.
pub fn escape_filter_path(path: &Path) -> String {
    let mut out = String::new();
    for ch in path.to_string_lossy().chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            ':' => out.push_str("\\:"),
            other => out.push(other),
        }
    }
    out
}

/// One ffmpeg pass that burns an ASS subtitle file into `video_path` and
/// muxes `audio_path`, re-encoding the video (libx264). Used instead of
/// [`mux_audio_command`] when the plan carries enabled subtitles.
pub fn subtitle_burn_command(
    video_path: &Path,
    ass_path: &Path,
    audio_path: &Path,
    output_path: &Path,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
        "-i".to_string(),
        audio_path.to_string_lossy().to_string(),
        "-vf".to_string(),
        format!("subtitles=filename='{}'", escape_filter_path(ass_path)),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "18".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-map".to_string(),
        "0:v".to_string(),
        "-map".to_string(),
        "1:a".to_string(),
        "-shortest".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}

#[derive(Debug)]
pub enum RenderError {
    Ffmpeg {
        command: Vec<String>,
        stderr: String,
    },
    Io(std::io::Error),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Ffmpeg { command, stderr } => {
                write!(
                    f,
                    "ffmpeg command failed (`{}`): {stderr}",
                    command.join(" ")
                )
            }
            RenderError::Io(e) => write!(
                f,
                "failed to run ffmpeg: {e} (is ffmpeg installed and on PATH?)"
            ),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        RenderError::Io(e)
    }
}

pub fn run_ffmpeg(args: &[String]) -> Result<(), RenderError> {
    let output = Command::new("ffmpeg").args(args).output()?;

    if !output.status.success() {
        return Err(RenderError::Ffmpeg {
            command: args.to_vec(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_for_maps_aspect_ratios() {
        assert_eq!(resolution_for(AspectRatio::Sixteen9), (1920, 1080));
        assert_eq!(resolution_for(AspectRatio::Nine16), (1080, 1920));
    }

    #[test]
    fn ken_burns_command_builds_expected_args_for_contain() {
        let args = ken_burns_command(
            Path::new("photo.jpg"),
            2.0,
            (1920, 1080),
            FitMode::Contain,
            Path::new("out.mp4"),
        );

        assert_eq!(
            args,
            vec![
                "-hide_banner".to_string(),
                "-y".to_string(),
                "-loop".to_string(),
                "1".to_string(),
                "-i".to_string(),
                "photo.jpg".to_string(),
                "-t".to_string(),
                "2.000".to_string(),
                "-vf".to_string(),
                "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2,scale=iw*4:ih*4:flags=lanczos,zoompan=z='1+0.15*on/119':d=120:s=1920x1080:fps=60:x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)'"
                    .to_string(),
                "-r".to_string(),
                "60".to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "out.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn ken_burns_command_builds_expected_args_for_cover() {
        let args = ken_burns_command(
            Path::new("photo.jpg"),
            2.0,
            (1920, 1080),
            FitMode::Cover,
            Path::new("out.mp4"),
        );

        assert_eq!(
            args,
            vec![
                "-hide_banner".to_string(),
                "-y".to_string(),
                "-loop".to_string(),
                "1".to_string(),
                "-i".to_string(),
                "photo.jpg".to_string(),
                "-t".to_string(),
                "2.000".to_string(),
                "-vf".to_string(),
                "scale=1920:1080:force_original_aspect_ratio=increase,crop=1920:1080,scale=iw*4:ih*4:flags=lanczos,zoompan=z='1+0.15*on/119':d=120:s=1920x1080:fps=60:x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)'"
                    .to_string(),
                "-r".to_string(),
                "60".to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "out.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn video_clip_command_builds_expected_args_for_contain() {
        let args = video_clip_command(
            Path::new("clip.mp4"),
            3.5,
            (1080, 1920),
            FitMode::Contain,
            Path::new("out.mp4"),
        );

        assert_eq!(
            args,
            vec![
                "-hide_banner".to_string(),
                "-y".to_string(),
                "-stream_loop".to_string(),
                "-1".to_string(),
                "-i".to_string(),
                "clip.mp4".to_string(),
                "-t".to_string(),
                "3.500".to_string(),
                "-vf".to_string(),
                "scale=1080:1920:force_original_aspect_ratio=decrease,pad=1080:1920:(ow-iw)/2:(oh-ih)/2"
                    .to_string(),
                "-an".to_string(),
                "-r".to_string(),
                "60".to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "out.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn video_clip_command_builds_expected_args_for_cover() {
        let args = video_clip_command(
            Path::new("clip.mp4"),
            3.5,
            (1080, 1920),
            FitMode::Cover,
            Path::new("out.mp4"),
        );

        assert_eq!(
            args,
            vec![
                "-hide_banner".to_string(),
                "-y".to_string(),
                "-stream_loop".to_string(),
                "-1".to_string(),
                "-i".to_string(),
                "clip.mp4".to_string(),
                "-t".to_string(),
                "3.500".to_string(),
                "-vf".to_string(),
                "scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920"
                    .to_string(),
                "-an".to_string(),
                "-r".to_string(),
                "60".to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "out.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn concat_list_content_formats_one_line_per_clip() {
        let clips = vec![PathBuf::from("beat-001.mp4"), PathBuf::from("beat-002.mp4")];

        let content = concat_list_content(&clips);

        assert_eq!(content, "file 'beat-001.mp4'\nfile 'beat-002.mp4'\n");
    }

    #[test]
    fn concat_command_builds_expected_args() {
        let args = concat_command(Path::new("list.txt"), Path::new("out.mp4"));

        assert_eq!(
            args,
            vec![
                "-hide_banner".to_string(),
                "-y".to_string(),
                "-f".to_string(),
                "concat".to_string(),
                "-safe".to_string(),
                "0".to_string(),
                "-i".to_string(),
                "list.txt".to_string(),
                "-c:v".to_string(),
                "copy".to_string(),
                "out.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn mux_audio_command_builds_expected_args() {
        let args = mux_audio_command(
            Path::new("video.mp4"),
            Path::new("audio.mp3"),
            Path::new("final.mp4"),
        );

        assert_eq!(
            args,
            vec![
                "-hide_banner".to_string(),
                "-y".to_string(),
                "-i".to_string(),
                "video.mp4".to_string(),
                "-i".to_string(),
                "audio.mp3".to_string(),
                "-c:v".to_string(),
                "copy".to_string(),
                "-c:a".to_string(),
                "aac".to_string(),
                "-map".to_string(),
                "0:v".to_string(),
                "-map".to_string(),
                "1:a".to_string(),
                "-shortest".to_string(),
                "final.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn escape_filter_path_escapes_colon_backslash_and_quote() {
        assert_eq!(
            escape_filter_path(Path::new("/tmp/a:b/subs.ass")),
            "/tmp/a\\:b/subs.ass"
        );
        assert_eq!(
            escape_filter_path(Path::new("/tmp/o'k/subs.ass")),
            "/tmp/o\\'k/subs.ass"
        );
        // Each backslash is doubled.
        assert_eq!(
            escape_filter_path(Path::new("/tmp/a\\b/subs.ass")),
            "/tmp/a\\\\b/subs.ass"
        );
    }

    #[test]
    fn escape_filter_path_passes_comma_through_unchanged() {
        // Inside the single-quoted filename form, `,` is literal.
        assert_eq!(
            escape_filter_path(Path::new("/tmp/v, 2026/subs.ass")),
            "/tmp/v, 2026/subs.ass"
        );

        let args = subtitle_burn_command(
            Path::new("concat.mp4"),
            Path::new("/tmp/v, 2026/subs.ass"),
            Path::new("script.mp3"),
            Path::new("final.mp4"),
        );
        let vf = &args[args.iter().position(|a| a == "-vf").unwrap() + 1];
        assert_eq!(vf, "subtitles=filename='/tmp/v, 2026/subs.ass'");
    }

    #[test]
    fn subtitle_burn_command_builds_expected_args() {
        let args = subtitle_burn_command(
            Path::new("concat.mp4"),
            Path::new("/tmp/r/subs.ass"),
            Path::new("script.mp3"),
            Path::new("final.mp4"),
        );

        assert_eq!(
            args,
            vec![
                "-hide_banner".to_string(),
                "-y".to_string(),
                "-i".to_string(),
                "concat.mp4".to_string(),
                "-i".to_string(),
                "script.mp3".to_string(),
                "-vf".to_string(),
                "subtitles=filename='/tmp/r/subs.ass'".to_string(),
                "-c:v".to_string(),
                "libx264".to_string(),
                "-preset".to_string(),
                "medium".to_string(),
                "-crf".to_string(),
                "18".to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "-c:a".to_string(),
                "aac".to_string(),
                "-map".to_string(),
                "0:v".to_string(),
                "-map".to_string(),
                "1:a".to_string(),
                "-shortest".to_string(),
                "final.mp4".to_string(),
            ]
        );
    }
}
