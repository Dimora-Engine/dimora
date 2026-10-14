//! Per-type event queues. Sending appends. Nothing is invoked, and an event does not change the world.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::World;

#[derive(Default)]
pub(crate) struct EventQueues {
    slots: HashMap<TypeId, Box<dyn EventSlot>>,
    /// Events that were still pending at the last [`World::clear_events`].
    discarded: u32,
}

trait EventSlot: Send {
    fn pending_len(&self) -> usize;
    fn promote(&mut self);
    fn clear(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

struct Typed<T> {
    pending: Vec<T>,
    visible: Vec<T>,
}

impl<T: Send + 'static> EventSlot for Typed<T> {
    fn pending_len(&self) -> usize {
        self.pending.len()
    }

    fn promote(&mut self) {
        self.visible.append(&mut self.pending);
    }

    fn clear(&mut self) {
        self.pending.clear();
        self.visible.clear();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl EventQueues {
    fn push<T: Send + 'static>(&mut self, event: T) {
        let slot = self
            .slots
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Typed::<T> { pending: Vec::new(), visible: Vec::new() }));
        let typed = slot.as_any_mut().downcast_mut::<Typed<T>>().expect("event slot type matches its TypeId");
        typed.pending.push(event);
    }

    fn visible<T: Clone + 'static>(&self) -> Vec<T> {
        self.slots
            .get(&TypeId::of::<T>())
            .and_then(|slot| slot.as_any().downcast_ref::<Typed<T>>())
            .map(|typed| typed.visible.clone())
            .unwrap_or_default()
    }

    fn promote(&mut self) {
        for slot in self.slots.values_mut() {
            slot.promote();
        }
    }

    fn clear(&mut self) {
        for slot in self.slots.values_mut() {
            slot.clear();
        }
    }

    /// Counts events that have not been promoted, then drops pending and visible events.
    fn discard_pending(&mut self) {
        self.discarded = self.slots.values().map(|slot| slot.pending_len() as u32).sum();
        self.clear();
    }
}

impl World {
    /// Appends `event` to its type's pending queue.
    ///
    /// It stays invisible to [`World::events`] until [`World::promote_events`]. Two sends of the
    /// same value are two events. `T: Send` keeps [`World`] `Send`.
    pub fn send<T: Send + 'static>(&self, event: T) {
        self.resource_mut::<EventQueues>().push(event);
    }

    /// Events of type `T` that have already been promoted, in send order.
    ///
    /// The returned vector is a copy, so reading does not drain the queue and does not hold a
    /// borrow across a later [`World::send`]. A type with no promoted events yields an empty vector.
    pub fn events<T: Clone + 'static>(&self) -> Vec<T> {
        self.resource::<EventQueues>().visible::<T>()
    }

    /// Moves every pending event into the visible buffer. The pending buffers start empty again.
    pub fn promote_events(&mut self) {
        self.resource_mut::<EventQueues>().promote();
    }

    /// Drops pending and visible events of every type.
    ///
    /// Events that were still pending are counted by [`World::discarded_events`] until
    /// [`World::clear_discarded_events`]. Promoted events were already visible, so they are not
    /// counted. The frame loop uses this after `input_clear`: a send in that stage is not delivered
    /// to a later system, and the count is how a caller sees that it was dropped.
    pub fn clear_events(&mut self) {
        self.resource_mut::<EventQueues>().discard_pending();
    }

    /// Events dropped by the latest [`World::clear_events`] because they had not been promoted.
    pub fn discarded_events(&self) -> u32 {
        self.resource::<EventQueues>().discarded
    }

    /// Zeroes [`World::discarded_events`]. The frame loop does this at the start of each step.
    pub fn clear_discarded_events(&mut self) {
        self.resource_mut::<EventQueues>().discarded = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Hit(u32);

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Footstep(&'static str);

    #[test]
    fn events_stay_pending_until_promote_and_keep_send_order_per_type() {
        let mut world = World::new();
        world.send(Hit(1));
        world.send(Footstep("left"));
        world.send(Hit(1));
        world.send(Hit(2));
        assert!(world.events::<Hit>().is_empty());
        assert!(world.events::<Footstep>().is_empty());

        world.promote_events();
        assert_eq!(world.events::<Hit>(), vec![Hit(1), Hit(1), Hit(2)]);
        assert_eq!(world.events::<Footstep>(), vec![Footstep("left")]);

        world.send(Hit(3));
        assert_eq!(world.events::<Hit>(), vec![Hit(1), Hit(1), Hit(2)], "a send is not visible in the same buffer");
        world.promote_events();
        assert_eq!(world.events::<Hit>(), vec![Hit(1), Hit(1), Hit(2), Hit(3)]);

        world.clear_events();
        assert!(world.events::<Hit>().is_empty());
        assert!(world.events::<Footstep>().is_empty());
        world.send(Hit(4));
        world.clear_events();
        world.promote_events();
        assert!(world.events::<Hit>().is_empty(), "a clear drops events that were still pending");
        assert_eq!(world.discarded_events(), 1, "the uncleared pending event is counted");

        world.clear_discarded_events();
        world.send(Hit(5));
        world.promote_events();
        world.send(Footstep("right"));
        world.clear_events();
        assert_eq!(world.discarded_events(), 1, "a promoted event was delivered and is not counted");
        assert!(world.events::<Hit>().is_empty());
        assert!(world.events::<Footstep>().is_empty());
    }
}
