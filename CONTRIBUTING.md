# Contributing to ai-vedit

Thanks for your interest in contributing! The CLI skeleton (M0) is in place —
read [README.md](README.md) for the architecture and [ROADMAP.md](ROADMAP.md)
for what's planned and what's not.

## Where to start

- **Backlog**: [GitHub Issues](../../issues) is the source of truth for
  concrete work items. Look for:
  - `good first issue` — small, self-contained, good entry point
  - `help wanted` — open and needs an owner
- **Bigger ideas**: if you want to propose something not on the
  [roadmap](ROADMAP.md) (e.g. a new asset-matching strategy, a new provider),
  open an issue to discuss it before writing code — this avoids wasted work
  if the direction doesn't fit the project's scope.
- **Questions**: open a [discussion](../../discussions) or an issue — no
  question is too small.

## Development setup

```bash
git clone <repo-url>
cd ai-vedit
cargo build
cargo test
```

You'll need:
- Rust (stable toolchain, via [rustup](https://rustup.rs))
- `ffmpeg` installed and on your `PATH`
- An `OPENAI_API_KEY` environment variable for anything touching
  transcription or the planning agent

## Code standards

- Format with `cargo fmt` before committing
- Lint with `cargo clippy` and address warnings
- Add or update tests for behavior you change
- Keep pull requests focused — one milestone item or issue per PR where
  possible, rather than bundling unrelated changes

## Submitting a change

1. Fork the repo and create a branch from `main`
2. Make your change, with tests where it makes sense
3. Ensure `cargo fmt --check`, `cargo clippy`, and `cargo test` all pass
4. Open a PR describing **what** changed and **why**, and link the issue it
   addresses (e.g. `Closes #12`)
5. Be responsive to review feedback — small back-and-forth is normal

## Design decisions

Architectural decisions (language choice, transcription/LLM providers, asset
categorization model, etc.) are documented in [README.md](README.md). If your
contribution challenges one of those decisions, raise it in an issue first
rather than changing it silently in a PR.

## License

By contributing, you agree that your contributions will be licensed under the
project's [MIT License](LICENSE).
