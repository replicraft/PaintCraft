//! `paintcraft-cli`: headless document creation, inspection, editing and rendering.
//! The desktop application is the separate `paintcraft` binary.

mod args;
mod commands;
mod report;

use std::io::Write as _;
use std::process::ExitCode;

use commands::Reply;

const HELP: &str = "\
paintcraft-cli <command> [options]

  new --width W --height H -o OUT.pcnd [--background '#RRGGBBAA'] [--policy ID] [--seed N] [--overwrite] [--json]
  inspect FILE.pcnd [--expect-input-sha256 HEX] [--json]
  schema batch|batch-v2 [--json]
  capabilities [--json]
  apply FILE.pcnd --operations BATCH.json -o OUT.pcnd [--expect-input-sha256 HEX] [--overwrite] [--json]
  render FILE.pcnd -o OUT.png [--expect-input-sha256 HEX] [--on-loss error] [--allow-loss flatten] [--overwrite] [--json]

Exit status: 0 ok; 1 internal; 2 invocation/schema; 3 unsupported; 4 stale or conflicting input;
5 resource/cancelled; 6 file/codec/save failure.
";

fn dispatch(argv: &[String]) -> Reply {
    let rest = &argv[argv.len().min(1)..];
    match argv.first().map(String::as_str) {
        Some("new") => commands::new(rest),
        Some("inspect") => commands::inspect(rest),
        Some("schema") => commands::schema(rest),
        Some("capabilities") => commands::capabilities(rest),
        Some("apply") => commands::apply(rest),
        Some("render") => commands::render(rest),
        Some("--help" | "-h" | "help") => Reply {
            stdout: HELP.as_bytes().to_vec(),
            stderr: String::new(),
            exit: 0,
        },
        Some("--version" | "-V") => Reply {
            stdout: format!("paintcraft-cli {}\n", env!("CARGO_PKG_VERSION")).into_bytes(),
            stderr: String::new(),
            exit: 0,
        },
        other => Reply {
            stdout: Vec::new(),
            stderr: format!(
                "error: {}\n\n{HELP}",
                other.map_or("missing command".to_owned(), |c| format!(
                    "unknown command {c:?}"
                ))
            ),
            exit: 2,
        },
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let reply = dispatch(&argv);
    // A broken pipe on stdout must not turn a completed command into a panic.
    let _ = std::io::stdout().write_all(&reply.stdout);
    eprint!("{}", reply.stderr);
    ExitCode::from(reply.exit)
}
