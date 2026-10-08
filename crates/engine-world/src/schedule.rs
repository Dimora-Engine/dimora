use crate::World;

type System = Box<dyn FnMut(&mut World)>;

/// Ordered list of systems run sequentially against a [`World`].
///
/// Deliberately minimal: no dependency graph, no parallelism. Systems are plain
/// `FnMut(&mut World)` so they are trivially testable by calling them directly.
#[derive(Default)]
pub struct Schedule {
    systems: Vec<(&'static str, System)>,
}

impl Schedule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_system(&mut self, name: &'static str, system: impl FnMut(&mut World) + 'static) -> &mut Self {
        self.systems.push((name, Box::new(system)));
        self
    }

    pub fn run(&mut self, world: &mut World) {
        for (_, system) in &mut self.systems {
            system(world);
        }
    }

    pub fn system_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.systems.iter().map(|(n, _)| *n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systems_run_in_insertion_order() {
        let mut w = World::new();
        w.insert_resource(Vec::<&'static str>::new());
        let mut s = Schedule::new();
        s.add_system("a", |w| w.resource_mut::<Vec<&str>>().push("a"))
            .add_system("b", |w| w.resource_mut::<Vec<&str>>().push("b"));
        s.run(&mut w);
        s.run(&mut w);
        assert_eq!(*w.resource::<Vec<&str>>(), vec!["a", "b", "a", "b"]);
        assert_eq!(s.system_names().collect::<Vec<_>>(), ["a", "b"]);
    }
}
