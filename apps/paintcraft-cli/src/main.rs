//! Headless PaintCraft automation. Commands share engine and file-outcome contracts.
mod commands;
mod report;

use clap::{Parser, Subcommand};
use std::io::Write as _;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    version,
    about,
    after_help = "Exit status: 0 ok; 1 internal; 2 invocation/schema; 3 unsupported; 4 conflict; 5 resource/cancelled; 6 file/codec/save failure."
)]
struct Cli {
    /// Print machine-readable JSON results.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve painting tools over the Model Context Protocol (stdio).
    Mcp(commands::mcp::Options),
    /// Flatten a layered project into its visible canvas composition.
    Flatten(commands::flatten::Options),
    /// Create a native document.
    New(commands::new::Options),
    /// Inspect a document and acquire its concurrency ticket.
    Inspect(commands::inspect::Options),
    /// Print an automation JSON schema.
    Schema(commands::schema::Options),
    /// Report implemented capabilities and qualification status.
    Capabilities(commands::capabilities::Options),
    /// Apply an atomic batch of edits.
    Apply(commands::apply::Options),
    /// Render a document to an image.
    Render(commands::render::Options),
    /// Export explicit ICO/CUR entries with independent sizes and encodings.
    ExportIcons(commands::export_icons::Options),
    /// Import an image as a native document.
    Import(commands::import::Options),
    /// Place an image file as a new layer, preserving the existing document.
    PasteImage(commands::paste_image::Options),
    /// Inspect and exchange palettes as assets, independently of image documents.
    Palette(commands::palette::Options),
    /// Inspect and exchange custom shape libraries as resources, independently of images.
    Shape(commands::shapes::Options),
    /// Inspect ABR assets or paint with an explicitly qualified native static tip.
    Brush(commands::brushes::Options),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let reply = match cli.command {
        Command::Mcp(options) => return commands::mcp::run(options),
        Command::Flatten(a) => commands::flatten::run(a, cli.json),
        Command::New(a) => commands::new::run(a, cli.json),
        Command::Inspect(a) => commands::inspect::run(a, cli.json),
        Command::Schema(a) => commands::schema::run(a, cli.json),
        Command::Capabilities(a) => commands::capabilities::run(a, cli.json),
        Command::Apply(a) => commands::apply::run(a, cli.json),
        Command::Render(a) => commands::render::run(a, cli.json),
        Command::ExportIcons(a) => commands::export_icons::run(a, cli.json),
        Command::Import(a) => commands::import::run(a, cli.json),
        Command::PasteImage(a) => commands::paste_image::run(a, cli.json),
        Command::Palette(a) => commands::palette::run(a, cli.json),
        Command::Shape(a) => commands::shapes::run(a, cli.json),
        Command::Brush(a) => commands::brushes::run(a, cli.json),
    };
    let _ = std::io::stdout().write_all(&reply.stdout);
    eprint!("{}", reply.stderr);
    ExitCode::from(reply.exit)
}
