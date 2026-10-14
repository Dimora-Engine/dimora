# ADR-0008: Deferred commands and per-frame events

* **Status:** Settled
* **Date:** 2026-10-14
* **Backlog id:** D-10
* **Decided by:** project owner (approved the proposed direction, with the ordering and lifetime rules below)
* **Affects:** `World` mutation, hierarchy, M4 scene load, M7 collision, M8 and M11 animation events, M18 scripts

## Context and problem

Systems mutate the `World` directly. A query borrows the `World`, so `spawn` and `despawn` cannot run
while it is open (KI-9). Resources are separate `RefCell`s: a double borrow of one resource panics.
There is no queue for a spawn during iteration, and no way to publish "this happened" to a later
system. Collision, scene load and scripts will all need both. Immediate callbacks would be a second
notification system.

## Requirements and constraints

* Flush uses the existing `World` methods (`spawn`, `despawn`, `insert`, `set_parent`,
  `detach_children`). It does not grow a second hierarchy.
* After a flush, every `Parent` points at a live entity that lists the child, and the reverse, which
  is the invariant `despawn` and `set_parent` already keep (ADR-0001).
* Command order is deterministic.
* Events are data. They are not callbacks and they do not change the world.
* The fixed stage may run more than once per `step` (ADR-0007).

## Options considered

### A. A FIFO command buffer flushed at stage boundaries, and typed event queues
* Advantages: one order; the query borrow stays shared; later stages can read what earlier stages emitted.
* Disadvantages: a system does not see its own structural changes until the flush.

### B. Observers invoked as soon as the event is sent
* Advantages: the listener runs immediately.
* Disadvantages: order depends on who registered, and a listener can re-enter the world. A framework the engine does not need yet.

### C. Wait until collision or scene load forces it
* Advantages: less code now.
* Disadvantages: those systems would invent their own queues.

## Decision and rationale

A.

### Commands

The first command set is `spawn`, `despawn`, `insert`, remove-one-component, `set_parent` and
`detach_children`. Each call appends to one FIFO queue.

Systems run in stage order. Calls append in call order. So a command from an earlier system is ahead
of a command from a later system, and two commands from one system stay in the order they were made.

The queue flushes at the end of each stage run, after every system in that run has returned. Each
`fixed_update` iteration is a run: it flushes before the next iteration, so a spawn in one tick
exists for the next tick. `update` and the other stages flush once per `step`.

Flush calls the existing methods in queue order. It does not stop at the first error and it does not
roll back. Each failure is kept, in order, for the caller to read, and then the next command runs.
A failed command changes nothing. A missing resource is still a panic: that is a programmer error,
the same as `World::resource` today. A command that names an entity which is not alive is a failure,
not a panic.

Consequences of that order, using the methods as they behave now:

* **Despawn, then something aimed at that entity.** `despawn` removes the subtree (ADR-0001). A later command whose target is gone fails. Commands earlier in the queue have already been applied.
* **Something aimed at an entity, then a despawn of it.** The earlier command runs while the entity is alive. The despawn then removes it.
* **Several commands, one entity.** They apply in queue order. The last successful write of a component is the one that remains. A remove followed by an insert leaves the component present.
* **Parent and child.** `set_parent` and `detach_children` are the hierarchy. A `set_parent` that would cycle, or that names a missing entity, fails and leaves the previous parent in place, because that is what `set_parent` does. Despawning a parent removes the children before any later command. A later command aimed at one of those children fails. A command aimed at a child and queued before the parent's despawn runs first.
* **Invariants.** They hold after the flush because each successful call preserves them and each failed call does not write.

A system may still call `spawn` and `despawn` directly when it does not hold a query. The queue is
how a structural change survives a query, and how stage order stays deterministic.

### Events

A typed queue. Sending appends. Nothing is invoked.

An event is not visible to any system in the stage that sent it, including the sender and systems
that run later in that same stage. It becomes visible when that stage run has finished, to systems
in later stages of the same `step`.

`fixed_update` is one stage even when it runs several times. Events sent in any iteration stay
invisible through the remaining iterations. They become visible to `update` and the stages after it.
They do not cross from one fixed iteration to the next. A fixed system that must react inside the
tick uses commands or writes a component, not an event.

The queues are cleared at the end of `step`, after `input_clear`, before the next `step`. They are
not cleared between fixed iterations.

`input_clear` is the last world stage. An event sent there has no later stage. It is cleared at the
end of `step` and no system reads it. A sender that needs a reader uses an earlier stage. Extract
and render do not read events.

Within one type, order is send order. The same event sent twice is two events. Events do not modify
the world.

## Consequences and follow-up

* M3a-3 implements the queue, the flush, and the event buffers. M3a-2's stages are what they flush against.
* M4, M7, M8 and M18 are consumers. They do not add observers.
* If a fixed-step consumer (M7 is the likely one) must see an event inside the same tick, that is a
  revisit of the fixed-iteration rule, with a failing test, not a silent change.

## Evidence and revisit conditions

Throwaway program, 2026-10-14, not in the tree. Against the current `World`:

* A `query` borrow and `resource_mut` on a queue resource succeed together. One command was appended during iteration. `resource_mut` takes `&World` and uses `RefCell`.
* `world.spawn` in that same loop does not compile: `E0502`, the query already holds the `World`. The flush has to run after the query is dropped, which a stage-end flush does.
* A FIFO flush that called the real `despawn`, `insert` and `set_parent` kept `Parent` / `Children` consistent for last-write-wins, despawn-then-touch (the touch failed), touch-then-despawn (the touch stuck, then the entity was gone), parent despawn then a child command (the child command failed), and a cycle (`set_parent` failed and the old parent remained).

Holding two `resource_mut` borrows of the same queue still panics. The implementation must not let a
`RefMut` of the queue escape across another borrow of it.

Reopen the visibility rule only with a concrete system that cannot be split across two stages.
