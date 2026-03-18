use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand};

fn cli_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Magenta.on_default().effects(Effects::BOLD))
        .usage(AnsiColor::Magenta.on_default().effects(Effects::BOLD))
        .literal(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
        .placeholder(AnsiColor::Cyan.on_default())
}

#[derive(Parser)]
#[command(
    name = "kasetto",
    version,
    color = clap::ColorChoice::Always,
    styles = cli_styles(),
    about = "sync and maintain local AI skill packs",
    long_about = "An extremely fast AI skills manager, written in Rust.",
    after_help = "\x1b[1;35mExamples:\x1b[0m\n  \x1b[90mkasetto sync\x1b[0m\n  \x1b[90mkasetto sync --config skills.config.yaml\x1b[0m\n  \x1b[90mkasetto sync --config https://example.com/skills.config.yaml --dry-run --verbose\x1b[0m\n  \x1b[90mkasetto list\x1b[0m\n  \x1b[90mkasetto list --json\x1b[0m\n  \x1b[90mkasetto doctor\x1b[0m\n  \x1b[90mkasetto doctor --json\x1b[0m"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(
        about = "Sync skills from configured sources",
        long_about = "Read configuration, discover requested skills, then install/update/remove local copies so destination matches config.\n\nUse --dry-run to preview changes without modifying files.",
        after_help = "\x1b[1;35mExamples:\x1b[0m\n  \x1b[90mkasetto sync\x1b[0m\n  \x1b[90mkasetto sync --dry-run --verbose\x1b[0m\n  \x1b[90mkasetto sync --config https://example.com/skills.config.yaml\x1b[0m"
    )]
    Sync {
        #[arg(long, default_value = "skills.config.yaml")]
        #[arg(
            help = "config path or HTTP(S) URL",
            long_help = "Configuration location. Supports:\n- local file path (default: skills.config.yaml)\n- HTTP(S) URL to a YAML config file"
        )]
        config: String,
        #[arg(long)]
        #[arg(help = "preview actions without changing files")]
        dry_run: bool,
        #[arg(long)]
        #[arg(help = "suppress non-error output")]
        quiet: bool,
        #[arg(long)]
        #[arg(help = "print final report as JSON")]
        json: bool,
        #[arg(long)]
        #[arg(help = "disable colors and animations")]
        plain: bool,
        #[arg(long)]
        #[arg(help = "print per-skill action list")]
        verbose: bool,
    },
    #[command(
        about = "List installed skills from manifest database",
        long_about = "Read installed skills from the local manifest database.\n\nIn interactive terminals, kasetto opens a navigable skill browser with a banner, list pane, and detail pane. Use --json for scripting."
    )]
    List {
        #[arg(long)]
        #[arg(help = "print installed skills as JSON")]
        json: bool,
    },
    #[command(
        about = "Run local diagnostics",
        long_about = "Inspect local kasetto setup, including version, manifest path, active installation paths, and failed skill installs from the latest sync report.",
        after_help = "\x1b[1;35mExamples:\x1b[0m\n  \x1b[90mkasetto doctor\x1b[0m\n  \x1b[90mkasetto doctor --json\x1b[0m"
    )]
    Doctor {
        #[arg(long)]
        #[arg(help = "print diagnostic output as JSON")]
        json: bool,
    },
}
