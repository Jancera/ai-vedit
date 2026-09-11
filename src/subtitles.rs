use crate::whisper::Segment;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Write as _;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
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
/// Each cue holds the largest run of whole words that greedy word-wrap
/// (the same rule [`wrap_text`] renders with) fits into `max_lines` lines
/// of at most `max_chars_per_line` characters; leftover words spill into
/// further cues. A single word longer than `max_chars_per_line` forms its
/// own cue and overflows on screen — that is unavoidable without breaking
/// the word.
///
/// When `max_duration` is `Some(limit > 0.0)`, any cue whose
/// character-proportional span still exceeds `limit` is divided into
/// equal-time sub-cues with its words apportioned across them.
pub fn segments_to_cues(
    segments: &[Segment],
    max_chars_per_line: u32,
    max_lines: u32,
    max_duration: Option<f64>,
) -> Vec<Cue> {
    let max_chars = max_chars_per_line.max(1) as usize;
    let max_lines = max_lines.max(1) as usize;
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

        let words: Vec<&str> = text.split_whitespace().collect();
        let mut groups: Vec<String> = Vec::new();
        let mut w = 0;
        while w < words.len() {
            let take = words_per_cue(&words[w..], max_chars, max_lines).max(1);
            groups.push(words[w..w + take].join(" "));
            w += take;
        }
        if groups.is_empty() {
            continue;
        }
        let total_chars: usize = groups
            .iter()
            .map(|g| g.chars().count())
            .sum::<usize>()
            .max(1);

        let mut cursor = segment.start;
        let last = groups.len() - 1;
        for (i, group) in groups.iter().enumerate() {
            let cue_end = if i == last {
                segment.end
            } else {
                let frac = group.chars().count() as f64 / total_chars as f64;
                cursor + duration * frac
            };
            push_bounded_cues(&mut cues, cursor, cue_end, group, max_duration);
            cursor = cue_end;
        }
    }

    cues
}

/// Greatest number of leading `words` that [`greedy_wrap`] packs into at
/// most `max_lines` lines of width `max_chars`. Always returns at least 1
/// for a non-empty slice — a lone over-long word still forms a group so
/// splitting terminates.
fn words_per_cue(words: &[&str], max_chars: usize, max_lines: usize) -> usize {
    let max_lines = max_lines.max(1);
    if words.is_empty() {
        return 0;
    }
    // `greedy_wrap` line count is non-decreasing as the prefix grows, so
    // binary-search the largest prefix that still fits.
    let (mut lo, mut hi) = (1usize, words.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if greedy_wrap(&words[..mid], max_chars).len() <= max_lines {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

/// Greedily packs `words` into lines no wider than `max_chars`: a new line
/// starts when appending the next word (plus a separating space) would
/// exceed `max_chars`. A word longer than `max_chars` still takes a line
/// of its own. This is the single wrap rule shared by cue splitting and
/// rendering.
fn greedy_wrap<'a>(words: &[&'a str], max_chars: usize) -> Vec<Vec<&'a str>> {
    let max_chars = max_chars.max(1);
    let mut lines: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut current_len = 0usize;

    for &word in words {
        let word_len = word.chars().count();
        let with_word = if current.is_empty() {
            word_len
        } else {
            current_len + 1 + word_len
        };
        if !current.is_empty() && with_word > max_chars {
            lines.push(std::mem::take(&mut current));
            current_len = 0;
        }
        if current.is_empty() {
            current_len = word_len;
        } else {
            current_len += 1 + word_len;
        }
        current.push(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Pushes `text` spanning `[start, end]` onto `cues`. When `max_duration`
/// is `Some(limit > 0.0)` and the span exceeds `limit`, the span is cut
/// into `ceil(span / limit)` equal-time sub-cues (each `<= limit`) with
/// `text`'s words apportioned across them by character count; if there are
/// fewer words than sub-cues the trailing sub-cues repeat the last word
/// group so the caption stays on screen for the whole span.
fn push_bounded_cues(
    cues: &mut Vec<Cue>,
    start: f64,
    end: f64,
    text: &str,
    max_duration: Option<f64>,
) {
    let limit = match max_duration {
        Some(limit) if limit > 0.0 => limit,
        _ => {
            cues.push(Cue {
                start,
                end,
                text: text.to_string(),
            });
            return;
        }
    };

    let span = end - start;
    if span <= limit {
        cues.push(Cue {
            start,
            end,
            text: text.to_string(),
        });
        return;
    }

    let sub_n = (span / limit).ceil().max(2.0) as usize;
    let parts = split_into_chunks(text, sub_n);
    if parts.is_empty() {
        cues.push(Cue {
            start,
            end,
            text: text.to_string(),
        });
        return;
    }

    let step = span / sub_n as f64;
    let mut sub_start = start;
    for j in 0..sub_n {
        let sub_end = if j == sub_n - 1 {
            end
        } else {
            sub_start + step
        };
        let part = parts.get(j).or_else(|| parts.last()).cloned().unwrap();
        cues.push(Cue {
            start: sub_start,
            end: sub_end,
            text: part,
        });
        sub_start = sub_end;
    }
}

/// Splits `text` into an even `n`-way partition of whole words: `n`
/// contiguous groups whose sizes differ by at most one word. Never
/// splits a word. Returns fewer than `n` groups only when there are
/// fewer than `n` words.
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
pub enum SubtitleError {
    InvalidColor(String),
    InvalidFont(String),
    ZeroField(&'static str),
}

impl fmt::Display for SubtitleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubtitleError::InvalidColor(v) => {
                write!(f, "primary_color must be \"#RRGGBB\" hex, got {v:?}")
            }
            SubtitleError::InvalidFont(v) => {
                write!(
                    f,
                    "font must be a plain font-family name (no comma or newline), got {v:?}"
                )
            }
            SubtitleError::ZeroField(name) => write!(f, "{name} must be greater than 0"),
        }
    }
}

impl std::error::Error for SubtitleError {}

/// `#RRGGBB` -> ASS `&H00BBGGRR` (opaque). Any other shape is an error.
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
fn wrap_text(text: &str, max_chars: usize, max_lines: usize) -> String {
    let max_lines = max_lines.max(1);
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return String::new();
    }

    let mut lines = greedy_wrap(&words, max_chars);
    // `segments_to_cues` sizes cues so this rarely fires; when it does
    // (e.g. a lone word wider than `max_chars`), fold everything from the
    // last kept line onward into one overflowing line rather than drop it.
    if lines.len() > max_lines {
        let tail: Vec<&str> = lines[max_lines - 1..].concat();
        lines.truncate(max_lines - 1);
        lines.push(tail);
    }

    lines
        .iter()
        .map(|line| line.join(" "))
        .collect::<Vec<_>>()
        .join("\\N")
}

impl SubtitleStyle {
    /// Rejects zero-valued layout fields and a malformed `primary_color`.
    pub fn validate(&self) -> Result<(), SubtitleError> {
        if self.font_size == 0 {
            return Err(SubtitleError::ZeroField("font_size"));
        }
        if self.max_lines == 0 {
            return Err(SubtitleError::ZeroField("max_lines"));
        }
        if self.max_chars_per_line == 0 {
            return Err(SubtitleError::ZeroField("max_chars_per_line"));
        }
        hex_to_ass_color(&self.primary_color)?;
        // `font` is interpolated raw into the comma-delimited ASS `Style:`
        // line; a comma or newline would break that line or inject extra
        // ASS directives, and an all-blank name draws nothing.
        if self.font.trim().is_empty()
            || self.font.contains(',')
            || self.font.contains('\n')
            || self.font.contains('\r')
        {
            return Err(SubtitleError::InvalidFont(self.font.clone()));
        }
        Ok(())
    }
}

/// Renders cues + style into a complete ASS document. A fixed 2px opaque
/// black outline is always applied. Validates the style first.
pub fn cues_to_ass(
    cues: &[Cue],
    style: &SubtitleStyle,
    resolution: (u32, u32),
) -> Result<String, SubtitleError> {
    style.validate()?;
    let primary = hex_to_ass_color(&style.primary_color)?;
    let (width, height) = resolution;
    let alignment = match style.position {
        SubtitlePosition::Bottom => 2,
        SubtitlePosition::Middle => 5,
        SubtitlePosition::Top => 8,
    };
    let bold = if style.bold { -1 } else { 0 };
    let italic = if style.italic { -1 } else { 0 };

    let mut out = String::new();
    out.push_str("[Script Info]\n");
    out.push_str("ScriptType: v4.00+\n");
    let _ = writeln!(out, "PlayResX: {width}");
    let _ = writeln!(out, "PlayResY: {height}");
    out.push_str("WrapStyle: 2\n");
    out.push_str("ScaledBorderAndShadow: yes\n\n");

    out.push_str("[V4+ Styles]\n");
    out.push_str(
        "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, \
         BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, \
         BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n",
    );
    let _ = writeln!(
        out,
        "Style: Default,{font},{size},{primary},{primary},&H00000000,&H00000000,{bold},{italic},\
         0,0,100,100,0,0,1,2,0,{alignment},40,40,{margin_v},1",
        font = style.font,
        size = style.font_size,
        margin_v = style.margin_vertical,
    );
    out.push('\n');

    out.push_str("[Events]\n");
    out.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );
    for cue in cues {
        let mut end = cue.end;
        if let Some(limit) = style.max_duration {
            if limit > 0.0 {
                end = end.min(cue.start + limit);
            }
        }
        let raw = if style.uppercase {
            cue.text.to_uppercase()
        } else {
            cue.text.clone()
        };
        let escaped = escape_ass_text(&raw);
        let text = wrap_text(
            &escaped,
            style.max_chars_per_line as usize,
            style.max_lines as usize,
        );
        let _ = writeln!(
            out,
            "Dialogue: 0,{start},{end},Default,,0,0,0,,{text}",
            start = format_ass_time(cue.start),
            end = format_ass_time(end),
        );
    }

    Ok(out)
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
    fn max_duration_bounds_every_cue_span_even_with_a_long_word() {
        // "dddd..." is one 30-char word that char-proportional timing would
        // otherwise stretch well past the 2s cap.
        let text = "a bb ccc dddddddddddddddddddddddddddddd";
        let cues = segments_to_cues(&[seg(0.0, 10.0, text)], 42, 2, Some(2.0));
        assert!(!cues.is_empty());
        for cue in &cues {
            assert!(
                cue.end - cue.start <= 2.0 + 1e-9,
                "cue {cue:?} spans more than the 2s cap"
            );
            assert!(cue.end >= cue.start);
        }
        // Sub-cues stay contiguous.
        for pair in cues.windows(2) {
            assert!((pair[0].end - pair[1].start).abs() < 1e-9);
        }
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

    /// Every emitted cue must wrap into at most `max_lines` lines, each no
    /// wider than `max_chars_per_line` — a lone word longer than the limit
    /// is the only allowed exception. Greedy word-wrap fits fewer chars
    /// than `max_chars_per_line * max_lines`, so a segment under that loose
    /// product can still overflow; this pins the real guarantee, plus
    /// contiguous timing and no lost words.
    #[test]
    fn every_cue_wraps_within_max_lines_and_max_chars() {
        let cases: &[(&str, u32, u32)] = &[
            // equal-length words under the loose 10*2 char product that
            // still cannot pack into two 10-char lines
            ("aaaaaa bbbbbb cccccc ffffff gggggg", 10, 2),
            // a realistic long sentence at the default 42 / 2 style
            (
                "The quarterly revenue figures exceeded every internal \
                 projection despite the supply chain disruptions that \
                 dominated the first half of the fiscal year",
                42,
                2,
            ),
            // a single word wider than the limit: its own cue, allowed to
            // overflow, but the surrounding words must still wrap cleanly
            ("intro antidisestablishmentarianism outro word", 12, 1),
        ];

        for &(text, max_chars, max_lines) in cases {
            let cues = segments_to_cues(&[seg(0.0, 6.0, text)], max_chars, max_lines, None);
            assert!(!cues.is_empty(), "no cues for {text:?}");

            for cue in &cues {
                let wrapped = wrap_text(&cue.text, max_chars as usize, max_lines as usize);
                let lines: Vec<&str> = wrapped.split("\\N").collect();
                assert!(
                    lines.len() <= max_lines as usize,
                    "cue {:?} wraps to {} lines (> {max_lines})",
                    cue.text,
                    lines.len()
                );
                for line in lines {
                    let single_word = !line.trim().contains(' ');
                    assert!(
                        line.chars().count() <= max_chars as usize || single_word,
                        "cue {:?} produced a {}-char line {line:?} (> {max_chars})",
                        cue.text,
                        line.chars().count()
                    );
                }
            }

            assert_eq!(cues.first().unwrap().start, 0.0);
            assert_eq!(cues.last().unwrap().end, 6.0);
            for pair in cues.windows(2) {
                assert!(
                    (pair[0].end - pair[1].start).abs() < 1e-9,
                    "cues not contiguous for {text:?}"
                );
            }
            let rejoined = cues
                .iter()
                .map(|c| c.text.clone())
                .collect::<Vec<_>>()
                .join(" ");
            assert_eq!(
                rejoined.split_whitespace().collect::<Vec<_>>(),
                text.split_whitespace().collect::<Vec<_>>(),
                "words lost or reordered for {text:?}"
            );
        }
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

    fn style_at(position: SubtitlePosition) -> SubtitleStyle {
        SubtitleStyle {
            position,
            ..SubtitleStyle::default()
        }
    }

    #[test]
    fn validate_rejects_zero_fields_and_bad_colour() {
        assert_eq!(
            SubtitleStyle {
                font_size: 0,
                ..SubtitleStyle::default()
            }
            .validate(),
            Err(SubtitleError::ZeroField("font_size"))
        );
        assert_eq!(
            SubtitleStyle {
                max_lines: 0,
                ..SubtitleStyle::default()
            }
            .validate(),
            Err(SubtitleError::ZeroField("max_lines"))
        );
        assert_eq!(
            SubtitleStyle {
                max_chars_per_line: 0,
                ..SubtitleStyle::default()
            }
            .validate(),
            Err(SubtitleError::ZeroField("max_chars_per_line"))
        );
        assert_eq!(
            SubtitleStyle {
                primary_color: "nope".to_string(),
                ..SubtitleStyle::default()
            }
            .validate(),
            Err(SubtitleError::InvalidColor("nope".to_string()))
        );
        assert!(SubtitleStyle::default().validate().is_ok());
    }

    #[test]
    fn validate_rejects_font_with_comma_newline_or_blank() {
        assert!(matches!(
            SubtitleStyle {
                font: "Arial, sans-serif".to_string(),
                ..SubtitleStyle::default()
            }
            .validate(),
            Err(SubtitleError::InvalidFont(_))
        ));
        assert!(matches!(
            SubtitleStyle {
                font: "Bad\nName".to_string(),
                ..SubtitleStyle::default()
            }
            .validate(),
            Err(SubtitleError::InvalidFont(_))
        ));
        assert!(matches!(
            SubtitleStyle {
                font: "  ".to_string(),
                ..SubtitleStyle::default()
            }
            .validate(),
            Err(SubtitleError::InvalidFont(_))
        ));
        assert!(SubtitleStyle::default().validate().is_ok());
    }

    #[test]
    fn ass_header_carries_resolution() {
        let ass = cues_to_ass(&[], &SubtitleStyle::default(), (1920, 1080)).unwrap();
        assert!(ass.contains("PlayResX: 1920"));
        assert!(ass.contains("PlayResY: 1080"));
        assert!(ass.contains("[V4+ Styles]"));
        assert!(ass.contains("[Events]"));
    }

    #[test]
    fn ass_style_line_reflects_position_and_flags() {
        let bottom = cues_to_ass(&[], &style_at(SubtitlePosition::Bottom), (1920, 1080)).unwrap();
        let middle = cues_to_ass(&[], &style_at(SubtitlePosition::Middle), (1920, 1080)).unwrap();
        let top = cues_to_ass(&[], &style_at(SubtitlePosition::Top), (1920, 1080)).unwrap();
        // Alignment is the 19th field on the "Style: Default,..." line.
        let alignment = |ass: &str| {
            ass.lines()
                .find(|l| l.starts_with("Style: Default,"))
                .unwrap()
                .trim_start_matches("Style: Default,")
                .split(',')
                .nth(17)
                .unwrap()
                .to_string()
        };
        assert_eq!(alignment(&bottom), "2");
        assert_eq!(alignment(&middle), "5");
        assert_eq!(alignment(&top), "8");

        let bold = SubtitleStyle {
            bold: true,
            italic: true,
            ..SubtitleStyle::default()
        };
        let ass = cues_to_ass(&[], &bold, (1920, 1080)).unwrap();
        let style_line = ass
            .lines()
            .find(|l| l.starts_with("Style: Default,"))
            .unwrap();
        // Bold is field 7, Italic field 8 (0-based 6 and 7) after "Style: Default,".
        let fields: Vec<&str> = style_line
            .trim_start_matches("Style: Default,")
            .split(',')
            .collect();
        assert_eq!(fields[6], "-1");
        assert_eq!(fields[7], "-1");
    }

    #[test]
    fn ass_dialogue_applies_uppercase_wrap_escape_and_timing() {
        let style = SubtitleStyle {
            uppercase: true,
            max_chars_per_line: 6,
            max_lines: 2,
            max_duration: Some(1.0),
            ..SubtitleStyle::default()
        };
        let cues = vec![Cue {
            start: 0.0,
            end: 5.0,
            text: "keep {it} short please".to_string(),
        }];
        let ass = cues_to_ass(&cues, &style, (1920, 1080)).unwrap();
        let dialogue = ass.lines().find(|l| l.starts_with("Dialogue:")).unwrap();
        assert!(dialogue.contains("KEEP")); // uppercased
        assert!(dialogue.contains("\\{IT\\}")); // escaped braces
        assert!(dialogue.contains("\\N")); // wrapped
        assert!(dialogue.starts_with("Dialogue: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,"));
        // max_duration clamp
    }

    #[test]
    fn cues_to_ass_propagates_validation_errors() {
        let bad = SubtitleStyle {
            font_size: 0,
            ..SubtitleStyle::default()
        };
        assert_eq!(
            cues_to_ass(&[], &bad, (1920, 1080)),
            Err(SubtitleError::ZeroField("font_size"))
        );
    }
}
