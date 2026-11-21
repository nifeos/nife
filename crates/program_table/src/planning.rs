//! **Planning a typed line against the catalog**: resolve the first word to its row, then hand the
//! row's manifest to the planner in `grant_plan`.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane milestone/1094-grant-plan-split.

use crate::{Endowment, ParsedInvocation, Program, Refusal, ShellHoldings, Streams, expansion};

/// Plan a line against the catalog: resolve its first word to a [`Program`] and plan it against
/// that row's manifest, with no operators on the line. [`plan_stage`] says what is checked.
///
/// Name: provisional, minted 2026-11-14 (UTC) by lane milestone/1094-grant-plan-split.
pub fn plan(
    run: &ParsedInvocation<'_>,
    shell_holdings: ShellHoldings,
    expanded: expansion::Expansion,
) -> Result<(Program, Endowment), Refusal> {
    plan_stage(run, shell_holdings, expanded, Streams::default())
}

/// **The word that will start an isolated child** (`swish -c <commands>`, milestone 52 (subshells without `fork`), DECISIONS
/// §160 (what a subshell copies)): this shell, run again explicitly, as a child whose endowment is derived from the
/// parent's. Reserved so the refusals that name the form and the line that types it agree, and so
/// that when the child is built nothing about the spelling moves.
///
/// Name: provisional (moved from `grant_plan` with the program table).
pub const ISOLATION_WORD: &[u8] = b"swish";

/// [`plan`], with the operators' answer folded in. The program is resolved first, because "there is
/// no such program" is a fact about the system, after the two answers that come before any name:
/// a line whose quotes do not make sense ([`ParsedInvocation::misquoted`]), since its first word is
/// not reliable either, and the reserved isolation form ([`ISOLATION_WORD`]), which has
/// no child behind it yet.
///
/// Name: provisional, minted 2026-11-14 (UTC) by lane milestone/1094-grant-plan-split.
pub fn plan_stage(
    run: &ParsedInvocation<'_>,
    shell_holdings: ShellHoldings,
    expanded: expansion::Expansion,
    streams: Streams,
) -> Result<(Program, Endowment), Refusal> {
    if let Some(r) = run.misquoted {
        return Err(r);
    }
    if run.program == ISOLATION_WORD {
        return Err(Refusal::IsolationNotBuilt);
    }
    let program = Program::from_name(run.program).ok_or(Refusal::NoSuchProgram)?;
    let endowment = grant_plan::plan_against_with_streams(
        run,
        program.manifest(),
        shell_holdings,
        expanded,
        streams,
    )?;
    Ok((program, endowment))
}
