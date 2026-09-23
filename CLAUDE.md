# CLAUDE.md

Powerline-style status line renderer for Claude Code, in Rust. It reads session JSON on stdin and prints ANSI-colored rows. See `README.md` for the input schema and the list of sections.

## Layout

- `src/main.rs`: the `claude_statusline` binary. It prints two rows, built by `location_sections` and `session_sections`, and each returns `(left, right)` section vectors.
- `src/bin/claude_subagent_statusline.rs`: the `claude_subagent_statusline` binary. It prints one `{"id","content"}` JSON line per subagent task.
- `src/render.rs`: shared colors, `Section`, powerline formatting, width and truncation helpers, and number/duration formatting.
- `src/lenient.rs`: deserializers that turn a field with an unexpected type into `None` (or drop a bad array element) instead of failing the parse.
- There is no lib crate. Both binaries pull the shared files in with `mod`, and the subagent binary uses `#[path = "../…"]`.
- Unit tests sit in a `#[cfg(test)] mod tests` at the bottom of each binary. `render.rs` helpers are tested from `main.rs`, so they don't run twice. `tests/cli.rs` runs the built binaries end to end.

## Commands

```sh
cargo test
cargo clippy --all-targets -- -D warnings   # what CI runs; pedantic is on
cargo fmt
cargo +1.88 test                            # the declared rust-version
./dev-install.sh                            # build, install to ~/.claude/, smoke-test
```

`.github/workflows/ci.yml` runs fmt, clippy, tests on Linux and macOS, and a test on the `rust-version` from `Cargo.toml`. Bump `rust-version` when using a newer language feature.

## Conventions

- Every input field is optional except `workspace.current_dir` and `model.display_name`. A section is hidden when its data is absent. Declare new fields as `Option<T>` with `#[serde(default, deserialize_with = "lenient::option")]`, so a type change in a future Claude Code release hides one section instead of the whole line.
- Don't read the clock inside section builders. Take `now: u64` (Unix seconds) as a parameter, as `session_sections` does, so countdowns stay testable.
- Measure width with `unicode-width` (terminal cells), never with `len()` or `chars().count()`. Right alignment depends on it.
- Right alignment uses the `COLUMNS` env var minus `CLAUDE_STATUSLINE_RIGHT_MARGIN` (default 5). When `COLUMNS` is missing or the groups would overlap, fall back to a single unpadded powerline. Never wrap.
- On any error, `main` prints `[statusline]`, writes the reason to stderr (which `claude --debug` logs), and exits normally. The subagent binary prints nothing, so Claude Code keeps its default rows. Don't panic, because the release profile uses `panic = "abort"`.
- `unsafe_code` is forbidden. Keep dependencies minimal: serde, serde_json, unicode-width.
- When a section, input field or env var changes, update `README.md` to match.

## Releasing

Bump `version` in `Cargo.toml`, run `cargo build --release` to refresh `Cargo.lock` (CI builds with `--locked`), then push a `v*` tag. `.github/workflows/release.yml` refuses to build if the tag and the crate version disagree.
