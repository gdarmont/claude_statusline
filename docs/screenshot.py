"""Regenerate docs/statusline.png, the README screenshot, from real output.

Feeds sample session JSON to the release binaries, turns their ANSI colors into
HTML set in MesloLGS NF, and screenshots it with headless Chrome. Needs
`cargo build --release` first, the MesloLGS NF font, and Chrome or Chromium
(or $CHROME pointing at one).

    python3 docs/screenshot.py
"""
import html, json, os, pathlib, re, shutil, subprocess, sys, tempfile, time

repo = pathlib.Path(__file__).resolve().parent.parent
bin_dir = repo / "target" / "release"
now = int(time.time())
columns = 160

# A fresh repo, so the location row doesn't depend on this checkout's name or branch
scratch = tempfile.TemporaryDirectory()
project = pathlib.Path(scratch.name) / "claude_statusline"
subprocess.run(["git", "init", "-q", "-b", "master", str(project)], check=True)

session = {
    "session_name": "demo",
    "model": {"display_name": "Opus"},
    "workspace": {"current_dir": str(project)},
    "cost": {"total_cost_usd": 0.42, "total_duration_ms": 754000,
             "total_lines_added": 156, "total_lines_removed": 23},
    "context_window": {"total_input_tokens": 74500, "context_window_size": 200000,
                       "used_percentage": 37.2,
                       "current_usage": {"cache_read_input_tokens": 61000}},
    "fast_mode": True, "effort": {"level": "high"}, "thinking": {"enabled": True},
    "rate_limits": {
        "five_hour": {"used_percentage": 23.5, "resets_at": now + 2 * 3600 + 13 * 60},
        "seven_day": {"used_percentage": 41.2, "resets_at": now + 3 * 86400 + 5 * 3600},
    },
    "prompt_cache": {"warm": True, "caching_observed": True, "ttl": "1h",
                     "expires_at": now + 42 * 60, "hit_ratio": 0.91,
                     "recache_tokens_if_cold": 45000},
    "agent": {"name": "security-reviewer"},
    "pr": {"number": 1234, "url": "https://github.com/o/r/pull/1234", "review_state": "approved"},
    "worktree": {"name": "my-feature", "original_branch": "master"},
}
tasks = {"columns": columns, "tasks": [
    {"id": "t1", "name": "Explore", "status": "running", "label": "scanning src/",
     "model": "claude-opus-5", "effort": "high", "contextWindowSize": 200000,
     "tokenCount": 12500, "startTime": (now - 134) * 1000,
     "tokenSamples": [0, 0, 500, 1500, 1500, 1600, 2600, 4100, 4300]},
    {"id": "t2", "name": "security-reviewer", "status": "completed", "label": "review install.sh",
     "model": "claude-sonnet-5-5", "effort": "medium", "contextWindowSize": 200000,
     "tokenCount": 48200},
]}

env = dict(os.environ, COLUMNS=str(columns), CLAUDE_STATUSLINE_UPDATE_CHECK="0")
rows = subprocess.run([f"{bin_dir}/claude_statusline"], input=json.dumps(session),
                      capture_output=True, text=True, env=env, check=True).stdout.splitlines()
out = subprocess.run([f"{bin_dir}/claude_subagent_statusline"], input=json.dumps(tasks),
                     capture_output=True, text=True, env=env, check=True).stdout
agent_rows = [json.loads(line)["content"] for line in out.splitlines()]

OSC8 = re.compile(r"\x1b\]8;[^\x07\x1b]*(?:\x07|\x1b\\)")
SGR = re.compile(r"\x1b\[([0-9;]*)m")

def to_html(line):
    line = OSC8.sub("", line)
    fg = bg = None
    bold = False
    parts, pos = [], 0
    def emit(text):
        if not text:
            return
        style = []
        if fg: style.append(f"color:rgb({fg})")
        if bg: style.append(f"background:rgb({bg})")
        if bold: style.append("font-weight:bold")
        parts.append(f'<span style="{";".join(style)}">{html.escape(text)}</span>')
    for m in SGR.finditer(line):
        emit(line[pos:m.start()])
        pos = m.end()
        codes = [int(c) if c else 0 for c in m.group(1).split(";")]
        i = 0
        while i < len(codes):
            c = codes[i]
            if c == 0: fg = bg = None; bold = False
            elif c == 1: bold = True
            elif c == 22: bold = False
            elif c == 39: fg = None
            elif c == 49: bg = None
            elif c in (38, 48) and i + 4 < len(codes) and codes[i + 1] == 2:
                rgb = ",".join(map(str, codes[i + 2:i + 5]))
                if c == 38: fg = rgb
                else: bg = rgb
                i += 4
            i += 1
    emit(line[pos:])
    return "".join(parts)

page = f"""<!doctype html><meta charset="utf-8">
<style>
  body {{ margin: 0; background: #1b1d23; }}
  .term {{ padding: 18px 22px; font: 15px/1.45 'MesloLGS NF', monospace; color: #d6d8de;
          white-space: pre; display: inline-block; }}
  .gap {{ height: 14px; }}
  .dim {{ color: #6b7080; }}
</style>
<div class="term">{"<br>".join(to_html(r) for r in rows)}<div class="gap"></div><span class="dim">agents</span><br>{"<br>".join(to_html(r) for r in agent_rows)}</div>
"""

chrome = os.environ.get("CHROME") or next(
    filter(None, map(shutil.which, ["google-chrome", "chromium", "chromium-browser"])), None)
if not chrome:
    sys.exit("no Chrome or Chromium found; set $CHROME")
page_path = pathlib.Path(scratch.name) / "statusline.html"
page_path.write_text(page)
# Wide enough for the right-aligned groups at these columns, tall enough for the rows
subprocess.run([chrome, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                "--force-device-scale-factor=2", "--window-size=1490,172",
                f"--screenshot={repo / 'docs' / 'statusline.png'}", page_path.as_uri()],
               check=True, capture_output=True)
