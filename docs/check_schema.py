"""Compare the payload Claude Code sends the status line with the last checked one.

Reads the `statusLine` schema, comments included, from the statusline-setup agent
prompt in the installed `claude` binary. Prints what changed since
docs/claude-code-payloads.txt, and exits 1 when anything did.

Once the renderer and README handle the changes, `--update` records this version
as checked, in the snapshot and in README § Input format:

    python3 docs/check_schema.py [path/to/claude] [--update]
"""
import difflib, mmap, pathlib, re, shutil, subprocess, sys, textwrap

repo = pathlib.Path(__file__).resolve().parent.parent
snapshot = repo / "docs" / "claude-code-payloads.txt"
readme = repo / "README.md"


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
            + statusline_schema(data) + "\n")

header = (f"# Payload Claude Code sends the status line command, last checked against {version}.\n"
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
    print(f"The payload is unchanged from {checked_version} to {version}.")

if update:
    snapshot.write_text(header + "\n" + body)
    text, count = re.subn(r"(last checked against Claude Code \*\*v)[\d.]+(\*\*)",
                          rf"\g<1>{version}\g<2>", readme.read_text())
    if count != 1:
        fail("README.md no longer says which version the schema was last checked against")
    readme.write_text(text)
    print(f"Recorded {version} as checked, in {snapshot.relative_to(repo)} and README.md.")
elif changed:
    print("\nOnce the renderer and README.md handle these changes, rerun with --update.")
    sys.exit(1)
