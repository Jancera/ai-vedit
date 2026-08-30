use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::AspectRatio;

pub const FPS: u32 = 60;

/// How far an asset's aspect ratio may drift from the output's and still
/// be scaled down to fit (rather than cropped/padded at native size).
const ASPECT_TOLERANCE: f64 = 0.01;

pub fn resolution_for(aspect: AspectRatio) -> (u32, u32) {
    match aspect {
        AspectRatio::Sixteen9 => (1920, 1080),
        AspectRatio::Nine16 => (1080, 1920),
    }
}

/// Fits an asset into a `width`x`height` frame.
///
/// When `asset` dimensions are known and the asset is larger than the
/// frame in both dimensions with an aspect ratio within [`ASPECT_TOLERANCE`]
/// of it, the asset is scaled down to the frame (the sub-1% overflow from
/// the tolerance is cropped from the center).
///
/// Otherwise the asset is never scaled: the excess is cropped, centered,
/// off any dimension where the asset is bigger than the frame, then padded
/// with black, centered, up to any dimension where it's smaller. An asset
/// smaller in both dimensions ends up centered on a black canvas at its
/// native size; one bigger in both gets a centered crop; a mismatched
/// asset gets both.
fn fit_filter(width: u32, height: u32, asset: Option<(u32, u32)>) -> String {
    if let Some((aw, ah)) = asset {
        if aw > width && ah > height && aspect_matches(width, height, aw, ah) {
            return format!(
                "scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height}:(iw-ow)/2:(ih-oh)/2"
            );
        }
    }
    format!(
        "crop=min(iw\\,{width}):min(ih\\,{height}),pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=black"
    )
}

/// Whether an `aw`x`ah` asset's aspect ratio is within `ASPECT_TOLERANCE`
/// of a `width`x`height` frame.
fn aspect_matches(width: u32, height: u32, aw: u32, ah: u32) -> bool {
    let target = width as f64 / height as f64;
    let actual = aw as f64 / ah as f64;
    (actual / target - 1.0).abs() <= ASPECT_TOLERANCE
}

pub fn ken_burns_command(
    image_path: &Path,
    duration: f64,
    resolution: (u32, u32),
    asset_dimensions: Option<(u32, u32)>,
    output_path: &Path,
) -> Vec<String> {
    let (width, height) = resolution;
    let frames = (duration * FPS as f64).round() as u64;
    // Last output frame index (0-based); guards against a degenerate
    // 0- or 1-frame beat so the division below never sees a zero
    // denominator.
    let last_frame = frames.saturating_sub(1).max(1);
    let fit = fit_filter(width, height, asset_dimensions);
    let zoompan = format!(
        "{fit},zoompan=z='1+0.15*on/{last_frame}':d={frames}:s={width}x{height}:fps={FPS}:x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)'"
    );

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
        zoompan,
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
    asset_dimensions: Option<(u32, u32)>,
    output_path: &Path,
) -> Vec<String> {
    let (width, height) = resolution;
    let fit = fit_filter(width, height, asset_dimensions);

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
        fit,
        "-an".to_string(),
        "-r".to_string(),
        FPS.to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}

/// ffprobe arguments that print an asset's first video stream dimensions
/// as `width,height` on one line.
pub fn probe_dimensions_command(path: &Path) -> Vec<String> {
    vec![
        "-v".to_string(),
        "error".to_string(),
        "-select_streams".to_string(),
        "v:0".to_string(),
        "-show_entries".to_string(),
        "stream=width,height".to_string(),
        "-of".to_string(),
        "csv=p=0".to_string(),
        path.to_string_lossy().to_string(),
    ]
}

/// Parses `width,height` (as emitted by [`probe_dimensions_command`]) into
/// a pixel pair. Returns `None` for any output that isn't two positive
/// integers, so callers fall back to native-resolution crop/pad.
fn parse_dimensions(stdout: &str) -> Option<(u32, u32)> {
    let line = stdout.lines().next()?;
    let (w, h) = line.trim().split_once(',')?;
    let w: u32 = w.trim().parse().ok()?;
    let h: u32 = h.trim().parse().ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Runs `ffprobe` to read an asset's pixel dimensions. Returns `None` if
/// ffprobe is missing, fails, or emits output we can't parse.
pub fn probe_dimensions(path: &Path) -> Option<(u32, u32)> {
    let output = Command::new("ffprobe")
        .args(probe_dimensions_command(path))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_dimensions(&String::from_utf8_lossy(&output.stdout))
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
    fn probe_dimensions_command_builds_expected_args() {
        let args = probe_dimensions_command(Path::new("photo.jpg"));

        assert_eq!(
            args,
            vec![
                "-v".to_string(),
                "error".to_string(),
                "-select_streams".to_string(),
                "v:0".to_string(),
                "-show_entries".to_string(),
                "stream=width,height".to_string(),
                "-of".to_string(),
                "csv=p=0".to_string(),
                "photo.jpg".to_string(),
            ]
        );
    }

    #[test]
    fn parse_dimensions_reads_width_and_height() {
        assert_eq!(parse_dimensions("3840,2160\n"), Some((3840, 2160)));
    }

    #[test]
    fn parse_dimensions_returns_none_for_unparseable_output() {
        assert_eq!(parse_dimensions(""), None);
        assert_eq!(parse_dimensions("N/A,N/A\n"), None);
        assert_eq!(parse_dimensions("1920\n"), None);
    }

    #[test]
    fn fit_filter_scales_down_oversized_same_aspect_asset() {
        let filter = fit_filter(1920, 1080, Some((3840, 2160)));

        assert_eq!(
            filter,
            "scale=1920:1080:force_original_aspect_ratio=increase,crop=1920:1080:(iw-ow)/2:(ih-oh)/2"
        );
    }

    const CROP_PAD_1080: &str =
        "crop=min(iw\\,1920):min(ih\\,1080),pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=black";

    #[test]
    fn fit_filter_scales_down_asset_within_aspect_tolerance() {
        // 3844x2160 is ~0.1% wider than 16:9 — still scaled, tiny overflow cropped.
        let filter = fit_filter(1920, 1080, Some((3844, 2160)));

        assert_eq!(
            filter,
            "scale=1920:1080:force_original_aspect_ratio=increase,crop=1920:1080:(iw-ow)/2:(ih-oh)/2"
        );
    }

    #[test]
    fn fit_filter_crops_and_pads_when_aspect_outside_tolerance() {
        // Oversized but square: nowhere near 16:9.
        assert_eq!(fit_filter(1920, 1080, Some((2160, 2160))), CROP_PAD_1080);
    }

    #[test]
    fn fit_filter_crops_and_pads_when_asset_not_larger_than_output() {
        // Same aspect, but only equal in size — no scaling headroom.
        assert_eq!(fit_filter(1920, 1080, Some((1920, 1080))), CROP_PAD_1080);
    }

    #[test]
    fn fit_filter_crops_and_pads_when_dimensions_unknown() {
        assert_eq!(fit_filter(1920, 1080, None), CROP_PAD_1080);
    }

    #[test]
    fn ken_burns_command_builds_expected_args() {
        let args = ken_burns_command(
            Path::new("photo.jpg"),
            2.0,
            (1920, 1080),
            None,
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
                "crop=min(iw\\,1920):min(ih\\,1080),pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=black,zoompan=z='1+0.15*on/119':d=120:s=1920x1080:fps=60:x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)'"
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
    fn ken_burns_command_scales_down_oversized_same_aspect_image() {
        let args = ken_burns_command(
            Path::new("photo.jpg"),
            2.0,
            (1920, 1080),
            Some((3840, 2160)),
            Path::new("out.mp4"),
        );

        let vf = &args[args.iter().position(|a| a == "-vf").unwrap() + 1];
        assert_eq!(
            vf,
            "scale=1920:1080:force_original_aspect_ratio=increase,crop=1920:1080:(iw-ow)/2:(ih-oh)/2,zoompan=z='1+0.15*on/119':d=120:s=1920x1080:fps=60:x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)'"
        );
    }

    #[test]
    fn video_clip_command_scales_down_oversized_same_aspect_video() {
        let args = video_clip_command(
            Path::new("clip.mp4"),
            3.5,
            (1920, 1080),
            Some((2560, 1440)),
            Path::new("out.mp4"),
        );

        let vf = &args[args.iter().position(|a| a == "-vf").unwrap() + 1];
        assert_eq!(
            vf,
            "scale=1920:1080:force_original_aspect_ratio=increase,crop=1920:1080:(iw-ow)/2:(ih-oh)/2"
        );
    }

    #[test]
    fn video_clip_command_builds_expected_args() {
        let args = video_clip_command(
            Path::new("clip.mp4"),
            3.5,
            (1080, 1920),
            None,
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
                "crop=min(iw\\,1080):min(ih\\,1920),pad=1080:1920:(ow-iw)/2:(oh-ih)/2:color=black"
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
}
