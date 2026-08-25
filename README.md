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
- Supported asset types: images (`.jpg`, `.png`, `.webp`) and video (`.mp4`).
- A reserved `general/` category acts as a fallback when no specific category fits a
  beat, or when a chosen category runs out of unused assets.
- Within a category, assets are picked by simple rotation (round-robin, preferring
  files not yet used) — no content-matching in the MVP.

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

- Validates that every category in the plan has at least one asset (falling back to
  `general/`, erroring if that's also empty).
- For each beat: picks an asset from its category.
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

M0 (CLI skeleton), M1 (transcription), and M2 (planning agent) are
implemented: the `plan` and `render` subcommands parse arguments and
validate config (`OPENAI_API_KEY`), and `plan` transcribes audio via the
OpenAI Whisper API (caching the result locally), then segments the
transcript into beats matched to asset categories via the OpenAI chat
completions API, writes `plan.json`, and prints a per-category time-budget
report. It doesn't yet do asset selection (M3) or rendering (M4). See
[ROADMAP.md](ROADMAP.md) for planned milestones and
[CONTRIBUTING.md](CONTRIBUTING.md) if you'd like to help.

## License

[MIT](LICENSE)
