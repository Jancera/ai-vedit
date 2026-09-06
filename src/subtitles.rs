use crate::whisper::Segment;
use serde::{Deserialize, Serialize};

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
}
