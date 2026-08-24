# Roadmap

`ai-vedit` is pre-implementation — the design in [README.md](README.md) is settled,
but no code exists yet. This roadmap breaks that design into milestones so
contributors can see what's planned and pick up a piece.

The canonical, up-to-date backlog lives in
[GitHub Issues](../../issues) (look for `good first issue` and `help wanted`
labels). This file tracks the higher-level milestones those issues fall under.

## Status: 🔴 Not started

## Milestones

### M0 — Project scaffolding
- Initialize the Rust project (`cargo`, workspace/crate layout)
- CLI argument parsing skeleton (`plan` / `render` subcommands, `--help`)
- Config/env handling (`OPENAI_API_KEY`)
- CI: `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt --check`

### M1 — Transcription
- OpenAI Whisper API client (`whisper-1`, `verbose_json` for timestamps)
- Local transcript caching (avoid re-billing on repeated runs)
- Error handling for API failures / invalid audio input

### M2 — Planning agent
- OpenAI chat completions client with structured output
- Beat segmentation from timestamped transcript (start/end time, description)
- Category matching: pick from existing asset folders, or propose new ones
- Per-category time-budget report (terminal output + saved plan file)
- Plan file format (JSON) — schema for beats + categories

### M3 — Asset library
- Category discovery by scanning `assets/<category>/` folders
- Supported types: `.jpg`, `.png`, `.webp`, `.mp4`
- Round-robin asset selection within a category (prefer unused files)
- `general/` fallback category handling

### M4 — Render pipeline
- ffmpeg invocation layer (shell-out + command construction)
- Image handling: Ken Burns (zoom/pan) hold for beat duration
- Video handling: trim to fit / loop if shorter than beat duration
- Concatenation of beat clips + narration audio overlay
- Aspect ratio config (`--aspect 16:9|9:16`, default 16:9 1920x1080)

### M5 — Polish
- Clear error messages (missing/empty categories, ffmpeg failures with the
  failing command shown)
- End-to-end integration test (sample audio + sample assets → rendered output)
- Usage docs / examples in README

## Ideas beyond the MVP (not committed yet)

These came up during design but were explicitly deferred to keep the MVP
scoped. Open an issue to propose one if you want to champion it:

- Pluggable transcription backends (e.g. local Whisper via whisper.cpp)
- Pluggable LLM providers for the planning agent (Anthropic, local models)
- AI-assisted asset matching within a category (instead of round-robin)
- Per-asset metadata/manifest files (tags, descriptions, usage history)
- Picture-in-picture / talking-head video support (currently audio-only input)
- Additional aspect ratios / custom resolutions beyond 16:9 and 9:16

## How milestones become issues

Each bullet above should become one or more GitHub Issues labeled with its
milestone (`M0`, `M1`, ...) plus `good first issue` where the scope is small
and self-contained. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to pick one
up.
