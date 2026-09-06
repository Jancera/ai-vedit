use crate::whisper::Segment;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subtitles {
    pub enabled: bool,
    pub style: SubtitleStyle,
    pub cues: Vec<Cue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleStyle {
    pub font: String,
    pub font_size: u32,
    pub primary_color: String,
    pub bold: bool,
    pub italic: bool,
    pub uppercase: bool,
    pub position: SubtitlePosition,
    pub margin_vertical: u32,
    pub max_chars_per_line: u32,
    pub max_lines: u32,
    #[serde(default)]
    pub max_duration: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitlePosition {
    Bottom,
    Middle,
    Top,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        SubtitleStyle {
            font: "DejaVu Sans".to_string(),
            font_size: 48,
            primary_color: "#FFFFFF".to_string(),
            bold: false,
            italic: false,
            uppercase: false,
            position: SubtitlePosition::Bottom,
            margin_vertical: 60,
            max_chars_per_line: 42,
            max_lines: 2,
            max_duration: None,
        }
    }
}

/// Splits transcript segments into caption-sized cues.
///
/// Whisper segments carry no per-word timestamps, so when a segment is
/// split its `[start, end]` span is divided across the chunks in
/// proportion to each chunk's character count. Cues within a segment are
/// contiguous and non-overlapping; gaps between segments are preserved.
///
/// `max_chars_per_line * max_lines` is the soft character target per cue.
/// When `max_duration` is `Some(limit > 0.0)`, extra splits are forced so
/// no cue spans more than `limit` seconds.
#[allow(dead_code)]
pub fn segments_to_cues(
    segments: &[Segment],
    max_chars_per_line: u32,
    max_lines: u32,
    max_duration: Option<f64>,
) -> Vec<Cue> {
    let target = (max_chars_per_line.max(1) as usize) * (max_lines.max(1) as usize);
    let mut cues = Vec::new();

    for segment in segments {
        let text = segment.text.trim();
        if text.is_empty() {
            continue;
        }
        let duration = segment.end - segment.start;
        if duration <= 0.0 {
            continue;
        }

        let by_chars = text.chars().count().div_ceil(target).max(1);
        let by_time = match max_duration {
            Some(limit) if limit > 0.0 => (duration / limit).ceil() as usize,
            _ => 1,
        };
        let n = by_chars.max(by_time).max(1);

        let chunks = split_into_chunks(text, n);
        if chunks.is_empty() {
            continue;
        }
        let total_chars: usize = chunks
            .iter()
            .map(|c| c.chars().count())
            .sum::<usize>()
            .max(1);

        let mut cursor = segment.start;
        let last = chunks.len() - 1;
        for (i, chunk) in chunks.iter().enumerate() {
            let cue_end = if i == last {
                segment.end
            } else {
                let frac = chunk.chars().count() as f64 / total_chars as f64;
                cursor + duration * frac
            };
            cues.push(Cue {
                start: cursor,
                end: cue_end,
                text: chunk.clone(),
            });
            cursor = cue_end;
        }
    }

    cues
}

/// Splits `text` into an even `n`-way partition of whole words: `n`
/// contiguous groups whose sizes differ by at most one word. Never
/// splits a word. Returns fewer than `n` groups only when there are
/// fewer than `n` words.
#[allow(dead_code)]
fn split_into_chunks(text: &str, n: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let n = n.clamp(1, words.len());
    let base = words.len() / n;
    let rem = words.len() % n;
    let mut chunks = Vec::with_capacity(n);
    let mut i = 0;
    for k in 0..n {
        let take = base + usize::from(k < rem);
        chunks.push(words[i..i + take].join(" "));
        i += take;
    }
    chunks
}

#[derive(Debug, PartialEq)]
#[allow(dead_code)]
pub enum SubtitleError {
    InvalidColor(String),
    ZeroField(&'static str),
}

impl fmt::Display for SubtitleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubtitleError::InvalidColor(v) => {
                write!(f, "primary_color must be \"#RRGGBB\" hex, got {v:?}")
            }
            SubtitleError::ZeroField(name) => write!(f, "{name} must be greater than 0"),
        }
    }
}

impl std::error::Error for SubtitleError {}

/// `#RRGGBB` -> ASS `&H00BBGGRR` (opaque). Any other shape is an error.
#[allow(dead_code)]
fn hex_to_ass_color(hex: &str) -> Result<String, SubtitleError> {
    let err = || SubtitleError::InvalidColor(hex.to_string());
    let body = hex.strip_prefix('#').ok_or_else(err)?;
    if body.len() != 6 || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(err());
    }
    let r = u8::from_str_radix(&body[0..2], 16).map_err(|_| err())?;
    let g = u8::from_str_radix(&body[2..4], 16).map_err(|_| err())?;
    let b = u8::from_str_radix(&body[4..6], 16).map_err(|_| err())?;
    Ok(format!("&H00{b:02X}{g:02X}{r:02X}"))
}

/// Seconds -> ASS timestamp `H:MM:SS.cc` (centiseconds). Negatives clamp to 0.
#[allow(dead_code)]
fn format_ass_time(seconds: f64) -> String {
    let cs = (seconds.max(0.0) * 100.0).round() as i64;
    let h = cs / 360_000;
    let m = (cs % 360_000) / 6_000;
    let s = (cs % 6_000) / 100;
    let c = cs % 100;
    format!("{h}:{m:02}:{s:02}.{c:02}")
}

/// Escapes ASS dialogue text: strips CR, turns LF into a space, and
/// backslash-escapes `\`, `{`, `}`.
#[allow(dead_code)]
fn escape_ass_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\r' => {}
            '\n' => out.push(' '),
            '\\' => out.push_str("\\\\"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            other => out.push(other),
        }
    }
    out
}

/// Greedy word wrap. Builds up to `max_lines` lines no wider than
/// `max_chars`; any remaining words are appended to the final line
/// (overflow beats truncation). Lines are joined with a literal `\N`.
#[allow(dead_code)]
fn wrap_text(text: &str, max_chars: usize, max_lines: usize) -> String {
    let max_chars = max_chars.max(1);
    let max_lines = max_lines.max(1);
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return String::new();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in words {
        let on_last_line = lines.len() + 1 >= max_lines;
        if on_last_line {
            if current.is_empty() {
                current.push_str(word);
            } else {
                current.push(' ');
                current.push_str(word);
            }
            continue;
        }
        let would_be = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if !current.is_empty() && would_be > max_chars {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        } else if current.is_empty() {
            current.push_str(word);
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines.join("\\N")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_style_has_expected_values() {
        let s = SubtitleStyle::default();
        assert_eq!(s.font, "DejaVu Sans");
        assert_eq!(s.font_size, 48);
        assert_eq!(s.primary_color, "#FFFFFF");
        assert_eq!(s.position, SubtitlePosition::Bottom);
        assert_eq!(s.margin_vertical, 60);
        assert_eq!(s.max_chars_per_line, 42);
        assert_eq!(s.max_lines, 2);
        assert_eq!(s.max_duration, None);
        assert!(!s.bold && !s.italic && !s.uppercase);
    }

    #[test]
    fn subtitles_round_trip_through_json() {
        let subs = Subtitles {
            enabled: true,
            style: SubtitleStyle::default(),
            cues: vec![Cue {
                start: 0.0,
                end: 2.5,
                text: "hello world".to_string(),
            }],
        };
        let json = serde_json::to_string(&subs).unwrap();
        let back: Subtitles = serde_json::from_str(&json).unwrap();
        assert_eq!(back, subs);
    }

    #[test]
    fn position_serializes_lowercase() {
        let json = serde_json::to_string(&SubtitlePosition::Middle).unwrap();
        assert_eq!(json, "\"middle\"");
    }

    fn seg(start: f64, end: f64, text: &str) -> Segment {
        Segment {
            start,
            end,
            text: text.to_string(),
        }
    }

    #[test]
    fn short_segment_becomes_one_cue_unchanged() {
        let cues = segments_to_cues(&[seg(1.0, 3.0, "a short line")], 42, 2, None);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "a short line");
        assert_eq!(cues[0].start, 1.0);
        assert_eq!(cues[0].end, 3.0);
    }

    #[test]
    fn long_segment_splits_on_word_boundaries_with_contiguous_times() {
        // 12 words; target = 5 chars * 1 line = 5 -> forces several chunks.
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let cues = segments_to_cues(&[seg(0.0, 12.0, text)], 5, 1, None);
        assert!(
            cues.len() >= 3,
            "expected multiple cues, got {}",
            cues.len()
        );
        // No word split: rejoining every cue's text reproduces the segment.
        let rejoined = cues
            .iter()
            .map(|c| c.text.clone())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(rejoined, text);
        // Times are contiguous and bounded by the segment.
        assert_eq!(cues.first().unwrap().start, 0.0);
        assert_eq!(cues.last().unwrap().end, 12.0);
        for pair in cues.windows(2) {
            assert!(
                (pair[0].end - pair[1].start).abs() < 1e-9,
                "cues must be contiguous"
            );
            assert!(pair[0].end >= pair[0].start);
        }
    }

    #[test]
    fn max_duration_forces_extra_splits() {
        // Short text (1 cue by chars) but 10s segment with a 2s cap -> >= 5 cues.
        let cues = segments_to_cues(&[seg(0.0, 10.0, "a b c d e f")], 42, 2, Some(2.0));
        assert!(
            cues.len() >= 5,
            "expected >= 5 cues from the 2s cap, got {}",
            cues.len()
        );
    }

    #[test]
    fn empty_and_zero_duration_segments_are_skipped() {
        let cues = segments_to_cues(
            &[
                seg(0.0, 2.0, "   "),
                seg(2.0, 2.0, "same time"),
                seg(2.0, 4.0, "kept"),
            ],
            42,
            2,
            None,
        );
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "kept");
    }

    #[test]
    fn single_word_longer_than_target_is_kept_whole() {
        let cues = segments_to_cues(&[seg(0.0, 2.0, "supercalifragilistic")], 5, 1, None);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "supercalifragilistic");
    }

    #[test]
    fn hex_color_converts_to_ass_bgr() {
        assert_eq!(hex_to_ass_color("#FFFFFF").unwrap(), "&H00FFFFFF");
        assert_eq!(hex_to_ass_color("#FF8800").unwrap(), "&H000088FF");
        assert_eq!(hex_to_ass_color("#000000").unwrap(), "&H00000000");
    }

    #[test]
    fn hex_color_rejects_bad_input() {
        assert_eq!(
            hex_to_ass_color("red"),
            Err(SubtitleError::InvalidColor("red".to_string()))
        );
        assert_eq!(
            hex_to_ass_color("#FFF"),
            Err(SubtitleError::InvalidColor("#FFF".to_string()))
        );
        assert_eq!(
            hex_to_ass_color("#GGGGGG"),
            Err(SubtitleError::InvalidColor("#GGGGGG".to_string()))
        );
    }

    #[test]
    fn ass_time_formats_hms_centiseconds() {
        assert_eq!(format_ass_time(0.0), "0:00:00.00");
        assert_eq!(format_ass_time(65.4), "0:01:05.40");
        assert_eq!(format_ass_time(3661.0), "1:01:01.00");
        assert_eq!(format_ass_time(-2.0), "0:00:00.00");
    }

    #[test]
    fn wrap_text_breaks_into_at_most_max_lines() {
        let wrapped = wrap_text("one two three four five six", 9, 2);
        let lines: Vec<&str> = wrapped.split("\\N").collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].chars().count() <= 9);
        // Overflow past max_lines lands on the final line rather than truncating.
        let long = wrap_text("aa bb cc dd ee ff gg hh ii jj", 5, 2);
        assert_eq!(long.split("\\N").count(), 2);
        assert!(long.contains("jj"));
    }

    #[test]
    fn wrap_text_single_line_when_max_lines_is_one() {
        assert_eq!(wrap_text("a b c d e", 3, 1), "a b c d e");
    }

    #[test]
    fn escape_ass_text_handles_braces_backslash_and_newlines() {
        assert_eq!(escape_ass_text("a {b} c"), "a \\{b\\} c");
        assert_eq!(escape_ass_text("x\\y"), "x\\\\y");
        assert_eq!(escape_ass_text("line1\r\nline2"), "line1 line2");
    }
}
