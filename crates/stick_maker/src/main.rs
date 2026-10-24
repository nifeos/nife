//! `stick_maker`: write nife's boot files to a USB stick or SD card. This file hands
//! `stick_maker::cli::run` the real machine and terminal. See notes/boot-stick.md.
//!
//! ```text
//! stick_maker                      # find the removable disks, ask which, confirm, write
//! stick_maker --list               # what would be offered, and nothing else
//! stick_maker --list --all         # ...and every disk that is not, with the reason
//! stick_maker --disk disk7 --yes   # no questions: copy onto disk7's FAT volume
//! stick_maker --disk disk7 --erase # no questions: erase disk7 if it is not FAT, then copy
//! ```

use std::process::ExitCode;

use stick_maker::cli::{self, Invocation, USAGE};
use stick_maker::embedded::{BUILD, FILES};
use stick_maker::host::{Machine, Terminal};

fn main() -> ExitCode {
    match cli::parse(std::env::args().skip(1)) {
        Ok(Invocation::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Invocation::Version) => {
            println!("stick_maker, nife build {BUILD}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("stick_maker: {message}");
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
        Ok(Invocation::Run(options)) => {
            match cli::run(&options, FILES, BUILD, &Machine, &mut Terminal) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => {
                    eprintln!("stick_maker: {message}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
