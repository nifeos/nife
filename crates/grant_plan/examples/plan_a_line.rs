//! **One command line, planned the way a shell plans it**: split the line at its operators, name
//! the files its redirections designate, parse each stage and plan it against the manifest of the
//! program it runs. The manifest is written out in full, because it is the whole of what the
//! planner knows about a program.
//!
//! Run it with `cargo run --example plan_a_line`, or pass a line of your own:
//! `cargo run --example plan_a_line -- 'count -v notes.txt > total.txt'`.

use grant_contract::{
    DirectoryGrantDeclaration, FileGrantDeclaration, IndexStoreDeclaration, InputDeclaration,
    Manifest, MemoryGrantDeclaration, OutputDeclaration, ProgramArgumentDeclaration, ShortOptions,
    StartupContract,
};
use grant_plan::expansion::Expansion;
use grant_plan::{
    CommandLine, Endowment, Refusal, ShellHoldings, Streams, input_file_grant, line,
    output_file_grant, parse, plan_against_with_streams,
};

/// The one program this example knows, `count`: it reads one file, named on the line, takes one
/// option, `-v`, and writes bytes, so `>` and `|` may carry its output. It declares nothing else,
/// so a line that tries to hand it more is refused.
const COUNT: Manifest = Manifest {
    program_argument: ProgramArgumentDeclaration::Forbidden,
    memory_grant: MemoryGrantDeclaration::Forbidden,
    file: FileGrantDeclaration::Required { writable: false },
    directory: DirectoryGrantDeclaration::Forbidden,
    short_options: ShortOptions::new(b"v"),
    output: OutputDeclaration::Bytes,
    input: InputDeclaration::Forbidden,
    reports: true,
    interruptible: false,
    clock: false,
    domain: false,
    configuration: false,
    entropy: false,
    network: false,
    machine: false,
    share: false,
    reboot: false,
    sync: false,
    installer: false,
    catalog: false,
    indexes: IndexStoreDeclaration::Forbidden,
    activation_directory: false,
    console_input: false,
    startup: StartupContract::Native,
};

/// Plan every stage of `text`, or answer the first refusal. The caller resolves a program's name
/// to its manifest; here the only name is `count`.
fn plan_line(text: &[u8], holdings: ShellHoldings) -> Result<Vec<Endowment>, Refusal> {
    let split = line::split(text)?;
    let output = split
        .output
        .map(|name| output_file_grant(name, holdings))
        .transpose()?;
    let input = split
        .input
        .map(|name| input_file_grant(name, holdings))
        .transpose()?;
    let mut plans = Vec::new();
    for (i, stage) in split.stages().iter().enumerate() {
        let CommandLine::Run(run) = parse(stage) else {
            return Err(Refusal::NotAPipelineStage);
        };
        if run.program != b"count" {
            return Err(Refusal::NoSuchProgram);
        }
        let streams = Streams {
            sink: split.stage_sink(i, output),
            source: split.stage_source(i, input),
            diagnostics: None,
        };
        plans.push(plan_against_with_streams(
            &run,
            COUNT,
            holdings,
            Expansion::none(),
            streams,
        )?);
    }
    Ok(plans)
}

fn main() {
    // A shell that holds a directory to grant from, standing at its root.
    let holdings = ShellHoldings {
        directory: true,
        ..ShellHoldings::default()
    };
    let typed: Vec<String> = std::env::args().skip(1).collect();
    let lines: Vec<String> = if typed.is_empty() {
        vec![
            "count -v notes.txt > total.txt".into(),
            "count -x notes.txt".into(),
            "count notes.txt extra.txt".into(),
        ]
    } else {
        typed
    };
    for text in &lines {
        println!("{text}");
        match plan_line(text.as_bytes(), holdings) {
            Ok(plans) => {
                for plan in plans {
                    println!("  grant the file: {:?}", plan.file);
                    println!("  options set:    {:#b}", plan.short_options);
                    println!("  output goes to: {:?}", plan.sink);
                }
            }
            Err(refusal) => println!("  refused: {}", refusal.message()),
        }
    }
}
