"""Compare the payloads Claude Code sends the status lines with the last checked ones.

Reads both from the installed `claude` binary: the `statusLine` schema, comments
included, from the statusline-setup agent prompt, and the `subagentStatusLine`
payload from the code that builds it, with minified names shown as `_` since they
change with every build. Prints what changed since docs/claude-code-payloads.txt,
and exits 1 when anything did.

Once the renderer and README handle the changes, `--update` records this version
as checked, in the snapshot and in README § Input format:

    python3 docs/check_schema.py [path/to/claude] [--update]
"""
import difflib, mmap, pathlib, re, shutil, subprocess, sys, textwrap

repo = pathlib.Path(__file__).resolve().parent.parent
snapshot = repo / "docs" / "claude-code-payloads.txt"
readme = repo / "README.md"
KEYWORDS = {"void", "true", "false", "null", "undefined", "typeof", "new", "in", "instanceof", "this"}


def fail(message):
    sys.exit(f"check_schema: {message}")


def statusline_schema(data):
    """The JSON-with-comments block the statusline-setup agent prompt documents."""
    anchor = data.find(b'"session_id": "string"')
    if anchor < 0:
        fail("statusLine schema not found; search the binary for recache_tokens_if_cold")
    brace = data.rfind(b"{\n", 0, anchor)
    line_start = data.rfind(b"\n", 0, brace) + 1
    indent = data[line_start:brace]
    end = data.find(b"\n" + indent + b"}", anchor)
    if indent.strip() or end < 0:
        fail("statusLine schema is not a block of its own any more")
    return textwrap.dedent(data[line_start:end + len(indent) + 2].decode())


def closing(text, start):
    """Index of the bracket closing the one at `start`, skipping string literals."""
    depth, quote, i = 0, None, start
    while i < len(text):
        c = text[i]
        if quote:
            if c == "\\":
                i += 1
            elif c == quote:
                quote = None
        elif c in "'\"`":
            quote = c
        elif c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    fail("unbalanced brackets in the subagent payload builder")


def opening(text, end):
    """Index of the `{` enclosing position `end`."""
    depth = 0
    for i in range(end - 1, -1, -1):
        if text[i] in ")]}":
            depth += 1
        elif text[i] in "([{":
            if depth == 0:
                return i
            depth -= 1
    fail("no object encloses the subagent tasks")


def entries(body):
    """Top-level `key:value` entries of an object literal's body."""
    parts, depth, quote, start = [], 0, None, 0
    for i, c in enumerate(body):
        if quote:
            if c == quote and body[i - 1] != "\\":
                quote = None
        elif c in "'\"`":
            quote = c
        elif c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif c == "," and depth == 0:
            parts.append(body[start:i])
            start = i + 1
    parts.append(body[start:])
    return [part for part in parts if part]


def normalize(expression, task):
    """Minified names as `_` and the task as `task`; property names stay."""
    def name(match):
        word = match.group(0)
        return "task" if word == task else word if word in KEYWORDS else "_"
    return re.sub(r"(?<![\w$.])[A-Za-z_$][\w$]*", name, expression)


def subagent_payload(data):
    """The object literal piped to `subagentStatusLine`, one field per line."""
    pattern = re.compile(rb"tasks:[\w$]+\.map\(\(?([\w$]+)\)?=>\(\{")
    for match in pattern.finditer(data):
        window_start = max(0, match.start() - 4096)
        text = data[window_start:match.end() + 8192].decode("latin-1")
        tasks_at = match.start() - window_start
        task_open = match.end() - window_start - 1
        task_body = text[task_open + 1:closing(text, task_open)]
        if "tokenSamples" not in task_body:
            continue
        task = match.group(1).decode()
        wrapper_open = opening(text, tasks_at)
        lines = ["{"]
        for entry in entries(text[wrapper_open + 1:closing(text, wrapper_open)]):
            if entry.startswith("tasks:"):
                lines.append("  tasks: [{")
                for field in entries(task_body):
                    key, _, value = field.partition(":")
                    lines.append(f"    {key}: {normalize(value, task)}")
                lines.append("  }]")
            elif entry.startswith("..."):
                lines.append(f"  ...{normalize(entry[3:], task)}")
            else:
                key, _, value = entry.partition(":")
                lines.append(f"  {key}: {normalize(value, task)}")
        lines.append("}")
        return "\n".join(lines) + "\n"
    fail("subagent payload builder not found; search the binary for tokenSamples")


args = [arg for arg in sys.argv[1:] if arg != "--update"]
update = "--update" in sys.argv[1:]
claude = args[0] if args else shutil.which("claude")
if not claude:
    fail("claude is not on PATH; pass the path to its binary")
claude = pathlib.Path(claude).resolve()
version = subprocess.run([str(claude), "--version"], capture_output=True, text=True,
                         check=True).stdout.split()[0]

with open(claude, "rb") as file, mmap.mmap(file.fileno(), 0, access=mmap.ACCESS_READ) as data:
    body = ("## statusLine: schema from the statusline-setup agent prompt\n"
            + statusline_schema(data)
            + "\n\n## subagentStatusLine: payload builder, minified names shown as _\n"
            + subagent_payload(data))

header = (f"# Payloads Claude Code sends the status line commands, last checked against {version}.\n"
          "# Written by docs/check_schema.py --update; run it without to see what changed.\n")
checked_version, checked_body = "none", ""
if snapshot.exists():
    old_header, _, checked_body = snapshot.read_text().partition("\n\n")
    found = re.search(r"last checked against (\d+(?:\.\d+)+)", old_header)
    checked_version = found.group(1) if found else "unknown"

changed = body != checked_body
if changed:
    sys.stdout.writelines(difflib.unified_diff(
        checked_body.splitlines(keepends=True), body.splitlines(keepends=True),
        f"checked ({checked_version})", f"installed ({version})"))
else:
    print(f"Both payloads are unchanged from {checked_version} to {version}.")

if update:
    snapshot.write_text(header + "\n" + body)
    text, count = re.subn(r"(last checked against Claude Code \*\*v)[\d.]+(\*\*)",
                          rf"\g<1>{version}\g<2>", readme.read_text())
    if count != 1:
        fail("README.md no longer says which version the schemas were last checked against")
    readme.write_text(text)
    print(f"Recorded {version} as checked, in {snapshot.relative_to(repo)} and README.md.")
elif changed:
    print("\nOnce the renderer and README.md handle these changes, rerun with --update.")
    sys.exit(1)
