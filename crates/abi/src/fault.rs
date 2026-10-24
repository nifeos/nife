/// **The spawn-slot convention** (§26 (the fault endpoint)): a supervised child is spawned with its
/// supervision endpoint in this reserved slot, by [`crate::thread_control_block::CAP_INSERT`] with
/// an explicit target or a [`Spawn`](../user/struct.Spawn.html) grant. At `START` the kernel reads
/// it: a `Rendezvous` capability makes the thread supervised, is recorded as its fault target and
/// is cleared, so the child cannot forge fault messages on it. An empty slot means unsupervised:
/// the thread dies and is reaped at once. It is the last slot, away from the low slots a child's
/// grants fill, so an unsupervised child never lands a working endpoint here by accident.
pub const FAULT_EP_SLOT: u64 = super::CAPABILITY_TABLE_SLOTS - 1;

// "The last slot" is the whole claim above; a wrong operator in the line before would still
// compile, so this makes it one that does not (a mutation-survivor triage).
const _: () = assert!(FAULT_EP_SLOT + 1 == super::CAPABILITY_TABLE_SLOTS);

/// `w0`: the thread faulted. **The message format** of §26 (the fault endpoint): a plain `RECEIVE`
/// of a death gets five words, and `r5` the label of §148 (resolves by asking the kernel).
/// ```text
///   w0  event  FAULT or EXIT
///   w1  tid    the dead thread's id, kernel-stamped: the kernel is the only sender here
///   w2  pc     the faulting instruction (0 for a clean exit)
///   w3  addr   the faulting address (0 for a clean exit, or a fault with no address)
///   w4  0      reserved; a fault-reply protocol arrives here additively (§26.4)
///   r5  label  x5, a5 or r9: the BADGE on the child's supervision capability, 0 if none
/// ```
pub const EVENT_FAULT: u64 = 1;
/// `w0`: the thread called `exit` (or was reaped), rather than faulting.
pub const EVENT_EXIT: u64 = 2;
