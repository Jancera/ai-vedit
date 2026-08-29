# ai-vedit

A CLI tool that turns a narrated audio script into an edited video by automatically
matching your narration to categorized visual assets (images and video clips) and
assembling the result with ffmpeg.

## Pipeline

```
script.mp3 --> transcribe --> agent plans shot list --> user fills asset categories --> render (ffmpeg)
```

1. **Input**: an MP3 recording of you reading a script (audio only, no video).
2. **Transcribe**: the audio is sent to the OpenAI Whisper API, producing a transcript
   with segment/word-level timestamps.
3. **Plan**: an AI agent (OpenAI API) reads the timestamped transcript and breaks it
   into narrative "beats" (start time, end time, description). For each beat it picks
   a matching asset category from your existing asset library, or proposes a new
   category name if nothing fits.
4. **Fill assets**: the plan reports how much total time each category needs
   (e.g. "city-broll: ~42s across 6 beats") so you know roughly how many assets to
   gather. You add image/video files into the corresponding category folders.
5. **Render**: the tool picks assets from each beat's category, fits them to the beat
   duration, and stitches everything together with ffmpeg over the original narration
   audio.

## Quickstart

Prerequisites:

- [`ffmpeg`](https://ffmpeg.org/) installed and on your `PATH`.
- The `ai-vedit` binary, built from source (no published binary yet):
  `cargo build --release` produces `target/release/ai-vedit`, or run
  `cargo install --path .` to put `ai-vedit` on your `PATH` so the commands
  below work as shown.
- An `OPENAI_API_KEY` (used for transcription and planning, set below).

```bash
export OPENAI_API_KEY=sk-...

mkdir -p assets/general
# `ai-vedit plan` creates assets/ and a folder per planned category
# automatically if they don't exist yet -- add images/videos to
# assets/general/, or drop them into the category folders it created
# for you, before running `ai-vedit render`

ai-vedit plan --audio script.mp3
# writes plan.json, prints a time-budget report, and lists any new
# categories the plan proposes that you still need to create

# fill in any newly-proposed category folders under assets/, then:
ai-vedit render --plan plan.json --out output.mp4
```

See [CLI usage](#cli-usage) below for the full flag reference.

## Tech stack

- **Language**: Rust
- **Transcription**: OpenAI Whisper API (`whisper-1`, `verbose_json` for timestamps)
- **Planning agent**: OpenAI API (chat completions with structured output)
- **Rendering**: shells out to the `ffmpeg` binary

## Asset library

Assets live in a plain folder structure — no manifest file required:

```
assets/
  city-broll/
    clip1.mp4
    photo1.jpg
  product-shots/
    shot1.png
    shot2.webp
  general/        # fallback category, used when nothing else fits
    filler1.mp4
```

- The folder name *is* the category name, discovered by scanning the directory.
- Supported asset types: images (`.jpg`, `.jpeg`, `.png`, `.webp`) and video (`.mp4`).
- A reserved `general/` category acts as a fallback when no specific category fits a
  beat, or when the chosen category has no assets at all (empty or missing folder).
- Within a category, assets are picked by simple rotation (round-robin): every file
  is used once before any file repeats — no content-matching in the MVP.

## CLI usage

### `plan`

```
ai-vedit plan --audio script.mp3 [--assets ./assets] [--aspect 16:9|9:16]
```

- Transcribes the audio (transcript is cached to disk at
  `<audio_dir>/.cache/<sha256-hash>.json` to avoid re-billing).
- Produces a shot list: beats with timestamps, assigned category, and a per-category
  time-budget report.
- Writes a plan file (JSON) and prints the time-budget report to the terminal.
- If new categories were proposed, lists which folders need to be created before
  rendering.
- `plan.json` is written to the current working directory and is overwritten on
  each run (there is no `--out` flag for `plan` yet).

### `render`

```
ai-vedit render --plan plan.json [--assets ./assets] [--out output.mp4] [--aspect 16:9|9:16]
```

- For each beat: picks an asset from its category, falling back to `general/` if the
  category has no assets, and erroring out at that beat if `general/` is also empty.
  - Images: held for the beat's duration with a Ken Burns (slow zoom/pan) effect.
  - Videos: trimmed to fit if longer than the beat, looped if shorter.
- Concatenates all beat clips, overlays the original narration audio, and encodes to
  the target resolution.

## Configuration

- `OPENAI_API_KEY` — required, read from the environment.
- Default output aspect ratio: **16:9 (1920x1080)**, overridable via `--aspect`.

## Error handling

- A category with no assets at render time fails with a message naming the category
  that needs assets (unless the `general/` fallback covers it).
- ffmpeg failures are surfaced with the failing command for debuggability.

## Status

M0 (CLI skeleton), M1 (transcription), M2 (planning agent), M3 (asset
library), M4 (render pipeline), and M5 (polish) are implemented: the `plan`
and `render` subcommands parse arguments and validate config
(`OPENAI_API_KEY`), and `plan` transcribes audio via the OpenAI Whisper API
(caching the result locally), then segments the transcript into beats
matched to asset categories via the OpenAI chat completions API, writes
`plan.json`, and prints a per-category time-budget report. `render` wires
up asset selection (file discovery + round-robin selection with `general/`
fallback) with an ffmpeg rendering pipeline: it generates a full video with
a Ken Burns effect for images, loop-and-trim for video clips, concatenates
all beat clips, and overlays the narration audio. M5 added case/whitespace-
tolerant category matching, symlink-following asset/category discovery,
clearer error messages, a real end-to-end integration test, and this
Quickstart. The full `plan` → `render` pipeline is functionally complete
end to end, completing the MVP (M0-M5). Anything further is tracked under
["Ideas beyond the MVP"](ROADMAP.md#ideas-beyond-the-mvp-not-committed-yet)
in [ROADMAP.md](ROADMAP.md). See [CONTRIBUTING.md](CONTRIBUTING.md) if
you'd like to help.

## License

[MIT](LICENSE)
