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
}
