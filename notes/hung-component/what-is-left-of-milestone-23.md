# The hung component: what it left of milestone 23

An appendix to [`notes/hung-component.md`](../hung-component.md), on the other two residuals of
milestone 23 (a capability-routed component OS with live replacement).

## What is left of milestone 23

State handoff is the crux. It is a serialise-old / absorb-new protocol over a supervisor-brokered
channel, which is a wire format and so calef's, and a hang makes it *harder* than the block says. A
hung component cannot be asked to serialize anything. So a state handoff protocol that only works
when the outgoing instance cooperates recovers a planned swap and not a failure, which is the case
it is most wanted for. Erlang/OTP's `code_change` has the same shape and the same hole.

Dependency-aware orchestration is built (`capability_plan::depends_on` and `dependents`,
notes/dependency-orchestration.md). The quiescence
protocol orchestration needs is telling a dependent to degrade before its dependency is swapped.
That is precisely the step a hang makes unavailable. So a dependency-aware supervisor still needs a
non-cooperative fallback for every edge in the graph, not just for the component it is replacing.

That fallback is not built. Both of its halves now exist for the component being replaced: a
supervisor notices by its bounded quiesce going unanswered, and ends a component that never
cooperates by destroying its region. Nothing yet applies either to a dependent.
