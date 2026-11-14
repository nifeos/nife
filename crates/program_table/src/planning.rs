//! **Planning a typed line against the catalog**: resolve the first word to its row, then hand the
//! row's manifest to the planner in `grant_plan`.
//!
//! Name: provisional, minted 2026-11-14 (UTC) by lane milestone/1094-grant-plan-split.

use crate::{Endowment, Holdings, ParsedInvocation, Program, Refusal, Streams, expand};

/// Plan a line against the catalog: resolve its first word to a [`Program`] and plan it against
/// that row's manifest, with no operators on the line. [`plan_stage`] says what is checked.
///
/// Name: provisional, minted 2026-11-14 (UTC) by lane milestone/1094-grant-plan-split.
pub fn plan(
    run: &ParsedInvocation<'_>,
    holds: Holdings,
    expanded: expand::Expansion,
) -> Result<(Program, Endowment), Refusal> {
    plan_stage(run, holds, expanded, Streams::default())
}

/// [`plan`], with the operators' answer folded in. The program is resolved first, because "there is
/// no such program" is a fact about the system, after the two answers that come before any name:
/// a line whose quotes do not make sense ([`ParsedInvocation::misquoted`]), since its first word is
/// not reliable either, and the reserved isolation form ([`grant_plan::ISOLATION_WORD`]), which has
/// no child behind it yet.
///
/// Name: provisional, minted 2026-11-14 (UTC) by lane milestone/1094-grant-plan-split.
pub fn plan_stage(
    run: &ParsedInvocation<'_>,
    holds: Holdings,
    expanded: expand::Expansion,
    streams: Streams,
) -> Result<(Program, Endowment), Refusal> {
    if let Some(r) = run.misquoted {
        return Err(r);
    }
    if run.program == grant_plan::ISOLATION_WORD {
        return Err(Refusal::IsolationNotBuilt);
    }
    let program = Program::from_name(run.program).ok_or(Refusal::NoSuchProgram)?;
    let endowment =
        grant_plan::plan_against_with(run, program.manifest(), holds, expanded, streams)?;
    Ok((program, endowment))
}
