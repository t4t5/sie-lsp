//! `sie` CLI — validates and dumps SIE files.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use sie_parser::{offset_to_line_col, parse, read_file, Severity};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("validate") => match args.get(2) {
            Some(path) => run_validate(path),
            None => {
                print_usage();
                ExitCode::from(2)
            }
        },
        Some("dump") => match args.get(2) {
            Some(path) => run_dump(path),
            None => {
                print_usage();
                ExitCode::from(2)
            }
        },
        Some("--help") | Some("-h") | Some("help") | None => {
            print_usage();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("sie: unknown subcommand `{other}`\n");
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn print_usage() {
    println!(
        "sie — SIE 4B file validator / dumper\n\
         \n\
         USAGE:\n\
           sie validate <file|->   parse and report diagnostics (exit 1 on errors)\n\
           sie dump     <file|->   parse and emit JSON to stdout\n\
           sie --help              show this message\n\
         \n\
         `-` as the path reads from stdin (assumed to be UTF-8)."
    );
}

fn read_source(path_arg: &str) -> std::io::Result<(String, String)> {
    if path_arg == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(("<stdin>".into(), buf))
    } else {
        let pb = PathBuf::from(path_arg);
        let s = read_file(&pb)?;
        Ok((path_arg.to_string(), s))
    }
}

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
        Severity::Hint => "hint",
    }
}

fn run_validate(path: &str) -> ExitCode {
    let (display, src) = match read_source(path) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("sie: {path}: {e}");
            return ExitCode::from(1);
        }
    };
    let out = parse(&src);
    let mut errors = 0u32;
    for d in &out.diagnostics {
        let (line, col) = offset_to_line_col(&src, d.span.byte_offset);
        eprintln!(
            "{}:{}:{}: {}: {}: {}",
            display,
            line + 1,
            col + 1,
            severity_str(d.severity),
            d.code,
            d.message
        );
        if d.severity == Severity::Error {
            errors += 1;
        }
    }
    if errors > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn run_dump(path: &str) -> ExitCode {
    let (_, src) = match read_source(path) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("sie: {path}: {e}");
            return ExitCode::from(1);
        }
    };
    let out = parse(&src);
    match serde_json::to_string_pretty(&out) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("sie: failed to serialise: {e}");
            ExitCode::from(1)
        }
    }
}
