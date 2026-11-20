# §101 appendix: what other operating systems do

Prior art for [§101 (notification objects: async multiplexing without
wait-any)](../0101-notification-objects.md). Moved out of that page on 2026-10-12 (UTC) under [§212
(a prose budget)](../0212-a-prose-budget-for-every-document.md). The main page is complete without
this one, which exists to verify or challenge it. No ruled word changed.

### seL4: notification objects, bound to TCBs (the model we adopt)

seL4 has a separate `Notification` object type: a data word (an array of binary semaphores) plus a
queue of TCBs. `seL4_Signal` ORs the capability's badge into the word and wakes the head waiter (or
counts the signal if nobody is waiting). `seL4_Wait` blocks if the word is zero, returns the word
and clears it if non-zero. `seL4_Poll` is the non-blocking version.

The key mechanism is binding (`seL4_TCB_BindNotification`): a notification bound to a TCB
delivers signals even while the thread is blocked in `RECV` on an endpoint. The receiver
distinguishes "this was IPC" from "this was a notification" by checking the badge. This is how seL4
does `select` without a `select` system_call: one blocking wait point that can be woken by either a
synchronous IPC message or an async notification, and the badge tells you which.

### Unix, Fuchsia, Mach and Redox

Cut on 2026-09-26 to fit the amendment under §212 (a prose budget)'s cap; the survey is
[at `256815e56`](https://github.com/nifeos/nife/blob/256815e5609cfb961ccb6526f8a0d3316e589402/design/decisions/0101-notification-objects.md#unixlinux-selectpollepollkqueue).
Mach's port sets are what seL4 replaced.
