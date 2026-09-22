# CLAUDE.md

Powerline-style status line renderer for Claude Code, in Rust. It reads session JSON on stdin and prints ANSI-colored rows. See `README.md` for the input schema and the list of sections.

## Layout

- `src/main.rs`: the `claude_statusline` binary. It prints two rows, built by `location_sections` and `session_sections`, and each returns `(left, right)` section vectors.
- `src/bin/claude_subagent_statusline.rs`: the `claude_subagent_statusline` binary. It prints one `{"id","content"}` JSON line per subagent task.
- `src/render.rs`: shared colors, `Section`, powerline formatting, width and truncation helpers, and number/duration formatting. There is no lib crate. Each binary pulls this file in with `mod render;`, and the subagent binary uses `#[path = "../render.rs"]`.

## Commands

```sh
cargo build --release
cargo clippy --all-targets   # clippy::all is deny, pedantic is warn
./dev-install.sh             # build, install to ~/.claude/, smoke-test
```

There are no automated tests. For a manual check, pipe JSON into the binaries (examples are under "Testing manually" in the README).

## Conventions

- Every input field is optional except `workspace.current_dir` and `model.display_name`. A section is hidden when its data is absent, so make new fields `Option<T>`.
- Measure width with `unicode-width` (terminal cells), never with `len()` or `chars().count()`. Right alignment depends on it.
- Right alignment uses the `COLUMNS` env var minus `CLAUDE_STATUSLINE_RIGHT_MARGIN` (default 5). When `COLUMNS` is missing or the groups would overlap, fall back to a single unpadded powerline. Never wrap.
- On any error, `main` prints `[statusline]` and exits normally. Don't panic, because the release profile uses `panic = "abort"`.
- `unsafe_code` is forbidden. Keep dependencies minimal: serde, serde_json, unicode-width.
- When a section, input field or env var changes, update `README.md` to match.

## Releasing

Bump `version` in `Cargo.toml`, run `cargo build --release` to refresh `Cargo.lock` (CI builds with `--locked`), then push a `v*` tag. `.github/workflows/release.yml` refuses to build if the tag and the crate version disagree.
