use serde::Deserialize;
use std::fmt::Write as _;
use std::io::Read;
use std::path::Path;
use std::process::Command;

const CONTEXT_WARNING_THRESHOLD: u8 = 59;

const LEFT_ROUND: &str = "\u{e0b6}";
const RIGHT_ARROW: &str = "\u{e0b0}";
const RIGHT_ROUND: &str = "\u{e0b4}";

#[derive(Deserialize)]
struct Input {
    workspace: Workspace,
    model: Model,
    cost: Option<Cost>,
    context_window: Option<ContextWindow>,
}

#[derive(Deserialize)]
struct Workspace {
    current_dir: String,
}

#[derive(Deserialize)]
struct Model {
    display_name: String,
}

#[derive(Deserialize)]
struct Cost {
    total_cost_usd: Option<f64>,
}

#[derive(Deserialize)]
struct ContextWindow {
    used_percentage: Option<f64>,
}

struct Rgb(u8, u8, u8);

impl Rgb {
    fn bg_ansi(&self) -> String {
        format!("\x1b[48;2;{};{};{}m", self.0, self.1, self.2)
    }

    fn fg_ansi(&self) -> String {
        format!("\x1b[38;2;{};{};{}m", self.0, self.1, self.2)
    }
}

const RESET: &str = "\x1b[0m";
const BLACK: Rgb = Rgb(0, 0, 0);
const WHITE: Rgb = Rgb(255, 255, 255);

struct Section {
    text: String,
    bg: Rgb,
    fg: Rgb,
}

fn ansi_styled(text: &str, bg: &Rgb, fg: &Rgb) -> String {
    format!("{RESET}{}{}{text}{RESET}", bg.bg_ansi(), fg.fg_ansi())
}

fn format_sections(sections: &[Section]) -> String {
    let mut out = String::new();

    let Some(first) = sections.first() else {
        return out;
    };

    // Leading round cap
    let _ = write!(out, "{}", ansi_styled(LEFT_ROUND, &BLACK, &first.bg));

    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            // Arrow separator: previous bg color -> current bg color
            let _ = write!(
                out,
                "{}",
                ansi_styled(RIGHT_ARROW, &sections[i].bg, &sections[i - 1].bg)
            );
        }
        let padded = format!(" {} ", section.text);
        let _ = write!(out, "{}", ansi_styled(&padded, &section.bg, &section.fg));
    }

    // Trailing round cap
    if let Some(last) = sections.last() {
        let _ = write!(
            out,
            "{}",
            ansi_styled(RIGHT_ROUND, &BLACK, &last.bg)
        );
    }

    out
}

fn get_git_branch() -> Option<String> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if branch.is_empty() { None } else { Some(branch) }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input_str = String::new();
    std::io::stdin().read_to_string(&mut input_str)?;
    let input: Input = serde_json::from_str(&input_str)?;

    let mut sections = Vec::new();

    // Directory name
    let dir_name = Path::new(&input.workspace.current_dir)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&input.workspace.current_dir);
    sections.push(Section {
        text: dir_name.to_string(),
        bg: Rgb(52, 86, 164),
        fg: WHITE,
    });

    // Git branch
    if let Some(branch) = get_git_branch() {
        sections.push(Section {
            text: branch,
            bg: Rgb(70, 107, 62),
            fg: WHITE,
        });
    }

    // Model
    sections.push(Section {
        text: input.model.display_name,
        bg: Rgb(68, 68, 68),
        fg: WHITE,
    });

    // Session cost
    if let Some(cost) = input.cost.and_then(|c| c.total_cost_usd)
        && cost > 0.0
    {
        sections.push(Section {
            text: format!("${cost:.2}"),
            bg: Rgb(128, 90, 140),
            fg: WHITE,
        });
    }

    // Context window usage
    if let Some(pct) = input.context_window.and_then(|c| c.used_percentage) {
        let percent = pct.round() as u8;
        let (bg, fg) = if percent > 80 {
            (Rgb(226, 0, 0), WHITE)
        } else if percent > CONTEXT_WARNING_THRESHOLD {
            (Rgb(204, 153, 0), BLACK)
        } else {
            (Rgb(70, 107, 62), WHITE)
        };
        sections.push(Section {
            text: format!("{percent}%"),
            bg,
            fg,
        });
    }

    println!("{}", format_sections(&sections));
    Ok(())
}

fn main() {
    if run().is_err() {
        println!("[statusline]");
    }
}
