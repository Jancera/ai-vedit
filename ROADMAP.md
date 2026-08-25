# Roadmap

The design in [README.md](README.md) is settled, and M0 (project scaffolding),
M1 (transcription), M2 (planning agent), M3 (asset library), and M4 (render
pipeline) are now implemented. This roadmap breaks the design into
milestones so contributors can see what's planned and pick up a piece.

The canonical, up-to-date backlog lives in
[GitHub Issues](../../issues) (look for `good first issue` and `help wanted`
labels). This file tracks the higher-level milestones those issues fall under.

## Status: 🟡 In progress — M0, M1, M2, M3, M4 complete

## Milestones

### M0 — Project scaffolding ✅ done
- [x] Initialize the Rust project (`cargo`, workspace/crate layout)
- [x] CLI argument parsing skeleton (`plan` / `render` subcommands, `--help`)
- [x] Config/env handling (`OPENAI_API_KEY`)
- [x] CI: `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt --check`

### M1 — Transcription ✅ done
- [x] OpenAI Whisper API client (`whisper-1`, `verbose_json` for timestamps)
- [x] Local transcript caching (avoid re-billing on repeated runs)
- [x] Error handling for API failures / invalid audio input

### M2 — Planning agent ✅ done
- [x] OpenAI chat completions client with structured output
- [x] Beat segmentation from timestamped transcript (start/end time, description)
- [x] Category matching: pick from existing asset folders, or propose new ones
- [x] Per-category time-budget report (printed to the terminal)
- [x] Plan file format (JSON) — schema for beats + categories

### M3 — Asset library ✅ done
- [x] Supported types: `.jpg`, `.jpeg`, `.png`, `.webp`, `.mp4`
- [x] Round-robin asset selection within a category (prefer unused files)
- [x] `general/` fallback category handling

### M4 — Render pipeline ✅ done
- [x] ffmpeg invocation layer (shell-out + command construction)
- [x] Image handling: Ken Burns (zoom/pan) hold for beat duration
- [x] Video handling: trim to fit / loop if shorter than beat duration
- [x] Concatenation of beat clips + narration audio overlay
- [x] Aspect ratio config (`--aspect 16:9|9:16`, default 16:9 1920x1080)

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
