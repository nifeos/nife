//! **The two-tier interrupt escalation policy**, §24 (interrupting the foreground process):
//! what the shell does at each step of watching a foreground job for `^C`.
//!
//! The crate root re-exports every public item (§272 (the grant-plan crate root splits into
//! private modules)).
//!
//! Name: provisional (milestone 842, 2026-10-11), as are its sibling modules' names.

// ---- the two-tier interrupt escalation policy (DECISIONS §24) ----

/// What the shell should do this step of watching a foreground job for `^C`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Nothing this step.
    None,
    /// Deliver the cooperative interrupt: ask the job to stop itself (the first tier).
    Cooperative,
    /// Tear the job down: it did not stop when asked (the forcible tier).
    Forcible,
}

/// Poll ticks the shell waits for a cooperative exit after the first `^C` before it escalates on a
/// timeout. This is the "shell-side timeout" DECISIONS §24 left to the shell: a job that ignores the
/// cooperative signal is still torn down without needing a second keystroke. In ticks (the shell's
/// own watch loop iterations), not wall time, so it is deterministic and testable without a clock.
pub const COOP_GRACE_TICKS: u32 = 200;

/// The two-tier escalation policy, as a small state machine the shell drives while a foreground job
/// runs. It holds where `^C` routing lives (DECISIONS §24: in the shell, userspace, because job
/// control is the shell's knowledge). Pure logic, host-tested, so the counts and the grace window
/// are pinned without an emulator.
///
/// The shell feeds it two events: [`on_interrupt`](Escalation::on_interrupt) when it observes a
/// fresh `^C`, and [`on_tick`](Escalation::on_tick) each watch-loop iteration. The first `^C` asks
/// the job to stop ([`Action::Cooperative`]); a second `^C`, or the grace window elapsing with no
/// clean exit, tears it down ([`Action::Forcible`]). After a forcible decision the machine is spent
/// and returns [`Action::None`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Escalation {
    interrupts: u32,
    grace_left: u32,
    coop_sent: bool,
    done: bool,
}

impl Escalation {
    pub fn new() -> Self {
        Escalation {
            interrupts: 0,
            grace_left: 0,
            coop_sent: false,
            done: false,
        }
    }

    /// A fresh `^C` was observed. The first asks the job to stop and arms the grace window; a second
    /// (while the same job is still foreground) escalates to a forcible teardown.
    pub fn on_interrupt(&mut self) -> Action {
        if self.done {
            return Action::None;
        }
        self.interrupts += 1;
        if self.interrupts == 1 {
            self.coop_sent = true;
            self.grace_left = COOP_GRACE_TICKS;
            Action::Cooperative
        } else {
            self.done = true;
            Action::Forcible
        }
    }

    /// A watch-loop tick with no new `^C`. Once the cooperative signal is out, the grace window
    /// counts down; when it reaches zero the job is torn down even without a second `^C`, so a job
    /// that ignores the cooperative signal does not hang the prompt forever.
    pub fn on_tick(&mut self) -> Action {
        if self.done || !self.coop_sent {
            return Action::None;
        }
        self.grace_left = self.grace_left.saturating_sub(1);
        if self.grace_left == 0 {
            self.done = true;
            Action::Forcible
        } else {
            Action::None
        }
    }

    /// Whether the policy has reached a forcible teardown (the shell stops watching after this).
    pub fn is_spent(&self) -> bool {
        self.done
    }
}

impl Default for Escalation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
