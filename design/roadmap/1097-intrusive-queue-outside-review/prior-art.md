# Prior art

An appendix to [the milestone](../1097-intrusive-queue-outside-review.md).

How the crate sits against what a reader of intrusive structures already knows. Recent downloads
are the last 90 days at the time of the review.

| where | shape | insert and remove | double-insert guard | what a reader coming from it will miss here |
|---|---|---|---|---|
| `intrusive-collections` 0.10 (9.0M recent, 26.2M all-time) | `SinglyLinkedList<A: Adapter>`, link is a `Cell`, adapter from a macro, pointer ops for `Box`, `Rc`, `UnsafeRef` | `push_front`, `pop_front`, cursors (`cursor_mut_from_ptr` then remove), `front`, `iter`, `take`, `clear` | runtime: a linked `Link` panics on a second insert (unverified exact behavior) | `front`, `iter`, cursors, `take`, a doubly linked `LinkedList` beside it |
| `cordyceps` 0.3 (4.5M recent, 9.9M all-time) | `List<T: Linked>` doubly linked, `Linked` is an `unsafe` trait that hands the list a pointer to the node's links without forming a reference to the node (unverified) | `push_back`, `pop_front`, O(1) `remove(NonNull)` (unverified) | debug assertions (unverified) | O(1) remove, iteration, a reference-free link trait |
| `pin-list` 0.1 (1.1M recent) | safe, `Pin`-based doubly linked list | cursor insert and remove | the type system | safety without `unsafe` at the call site; this crate puts one `unsafe` at the mint |
| Linux `list_head` | doubly linked, two pointers in the node | `list_add_tail`, `list_del` O(1), `list_for_each` | `CONFIG_DEBUG_LIST` | O(1) delete, iteration |
| FreeBSD `queue(3)` `STAILQ` | singly linked tail queue, the exact shape of this crate | `STAILQ_INSERT_TAIL`, `STAILQ_REMOVE_HEAD`, `STAILQ_REMOVE` O(n), `STAILQ_CONCAT`, `STAILQ_FOREACH` | none | `REMOVE`, `CONCAT`, `FOREACH`, `INSERT_HEAD` |
| seL4 `tcb_queue_t` | doubly linked through `tcbSchedNext` and `tcbSchedPrev` in the TCB | enqueue, O(1) dequeue of any thread | none | O(1) dequeue of a specific thread |

Where this crate differs: a move-only `Unqueued<T>` token that is the proof a node is off every
queue, so a second push does not compile rather than panicking. It has an O(1) `len`. One Kani harness
covers every push and pop interleaving to its bound, shipped with the patch that turns it red. It has no
dependencies, and `#[repr(transparent)]` makes `Option<Unqueued<T>>` one word. What it costs is a
method surface of four. There is no way to take a specific node out and no way to look at a node without
popping it. The singly linked shape makes any targeted unlink a walk from the head. The
concrete example is in the consumer: `inter_process_communication` writes the same
drain-and-rebuild loop five times to do what `STAILQ_REMOVE` and `cursor_mut_from_ptr` do once.
