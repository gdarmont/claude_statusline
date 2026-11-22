"""Compare what the status line reads from Claude Code with what was last checked.

Reads the `statusLine` schema, comments included, from the statusline-setup agent
prompt in the installed `claude` binary. The account section reads a file outside
that payload, so it also looks in the binary for where Claude Code keeps the login.
Prints what changed since docs/claude-code-payloads.txt, and exits 1 when anything
did. It then checks that this machine's login has the fields src/account.rs reads,
without printing them.

Once the renderer and README handle the changes, `--update` records this version
as checked, in the snapshot and in README § Input format:

    python3 docs/check_schema.py [path/to/claude] [--update]
"""
import difflib, json, mmap, os, pathlib, re, shutil, subprocess, sys, textwrap

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


# What src/account.rs relies on to find the login, as it appears in the minified binary
ACCOUNT_LOOKUPS = [
    ("config file: .claude${suffix}.json in $CLAUDE_CONFIG_DIR, else the home directory",
     rb"process\.env\.CLAUDE_CONFIG_DIR\|\|[\w$]+\(\),`\.claude\$\{[\w$]+\(\)\}\.json`"),
    ("legacy config file: .config.json in the config directory, read first",
     re.escape(b'(),".config.json")')),
    ("suffix: -custom-oauth when CLAUDE_CODE_CUSTOM_OAUTH_URL is set",
     re.escape(b'CLAUDE_CODE_CUSTOM_OAUTH_URL)return"-custom-oauth"')),
    ("login: oauthAccount", rb"\.oauthAccount\b"),
    ("email: emailAddress", rb"\bemailAddress\b"),
    ("organization: organizationName", rb"\borganizationName\b"),
]
ACCOUNT_FIELDS = ("emailAddress", "organizationName")


def account_lookups(data):
    """One line per lookup, saying whether the binary still has it."""
    return "".join(f"{'found' if re.search(pattern, data) else 'MISSING':<8} {label}\n"
                   for label, pattern in ACCOUNT_LOOKUPS)


def local_login():
    """None when this machine's login has the fields the account section reads, else why not.

    Finds the config file the way src/account.rs does. Never prints what the fields hold.
    """
    env = lambda name: os.environ.get(name) or ""
    config_dir, home = env("CLAUDE_CONFIG_DIR"), env("HOME")
    suffix = "-custom-oauth" if env("CLAUDE_CODE_CUSTOM_OAUTH_URL") else ""
    for path in (pathlib.Path(config_dir or pathlib.Path(home, ".claude"), ".config.json"),
                 pathlib.Path(config_dir or home, f".claude{suffix}.json")):
        try:
            text = path.read_text()
        except OSError:
            continue
        if text.strip():
            break
    else:
        print("Account: skipped, no Claude Code config found.")
        return None
    try:
        account = json.loads(text).get("oauthAccount")
    except (ValueError, AttributeError):
        return f"{path} is not a JSON object"
    if account is None:
        print(f"Account: skipped, {path} holds no login.")
        return None
    if not isinstance(account, dict):
        return f"oauthAccount in {path} is not an object"
    missing = [key for key in ACCOUNT_FIELDS if not isinstance(account.get(key), str)]
    if missing:
        return f"oauthAccount in {path} has no string {' or '.join(missing)}"
    print(f"Account: the login in {path} has {' and '.join(ACCOUNT_FIELDS)}.")
    return None


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
            + statusline_schema(data) + "\n"
            + "\n## account: where Claude Code keeps the login src/account.rs reads\n"
            + account_lookups(data))

header = (f"# Payload Claude Code sends the status line command, and where it keeps the login,\n"
          f"# last checked against {version}.\n"
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
    print(f"The payload and the login lookup are unchanged from {checked_version} to {version}.")

login_problem = local_login()
if login_problem:
    print(f"\nAccount: {login_problem}. The account section hides itself until"
          " src/account.rs reads the new layout.")
    if update:
        fail("not recording this version while the account section can't read the login")
    sys.exit(1)

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
