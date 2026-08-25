//! The `gitxtend` binary — a shim over [`gitxtend::cli::run`].
//!
//! Everything the command does lives in the library so the Python console
//! script (`gitxtend._cli`) executes the same code path. This file only moves
//! bytes between that function and the process's real streams.

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = gitxtend::cli::run(&args);

    print!("{}", outcome.stdout);
    // Flush explicitly: `print!` leaves stdout line-buffered, and the exit code
    // path below must not race the write when stdout is a pipe.
    let _ = std::io::stdout().flush();
    eprint!("{}", outcome.stderr);

    ExitCode::from(outcome.code)
}
