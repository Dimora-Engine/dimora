use std::collections::{BTreeSet, HashMap, HashSet};

use crate::World;

type System = Box<dyn FnMut(&mut World)>;

/// Why a schedule could not accept a system or a constraint.
///
/// A failed call leaves the schedule as it was. [`Schedule::run`] only executes an order that
/// was accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    /// `add_system` was called with a name already in this schedule.
    DuplicateName(&'static str),
    /// `before` / `after` named a system that is not in this schedule.
    UnknownSystem(&'static str),
    /// A constraint names a target that is not in this schedule.
    MissingTarget { system: &'static str, target: &'static str },
    /// A system was constrained against its own name.
    SelfDependency(&'static str),
    /// The constraints cannot be ordered. Names are in registration order.
    Cycle(Vec<&'static str>),
}

impl core::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ScheduleError::DuplicateName(name) => write!(f, "system `{name}` is already registered"),
            ScheduleError::UnknownSystem(name) => write!(f, "no system named `{name}` is in this schedule"),
            ScheduleError::MissingTarget { system, target } => {
                write!(f, "system `{system}` depends on `{target}`, which is not in this schedule")
            }
            ScheduleError::SelfDependency(name) => {
                write!(f, "system `{name}` cannot be ordered relative to itself")
            }
            ScheduleError::Cycle(names) => {
                let list = names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ");
                write!(f, "system order contains a cycle involving {list}")
            }
        }
    }
}

impl std::error::Error for ScheduleError {}

struct Entry {
    name: &'static str,
    system: System,
    /// This system runs before each of these names.
    before: Vec<&'static str>,
    /// This system runs after each of these names.
    after: Vec<&'static str>,
}

/// Ordered list of systems run sequentially against a [`World`].
///
/// Systems with no `before` / `after` constraint keep registration order relative to each other.
/// Constraints are a precedence graph. When several systems are ready, the one registered
/// earlier runs next. That rule picks one order whenever more than one order would satisfy the
/// constraints. A cycle, a missing target, a self-dependency or a duplicate name is rejected and
/// the schedule is left unchanged.
///
/// Single-threaded: systems are `FnMut(&mut World)` and run on the caller of [`Schedule::run`].
#[derive(Default)]
pub struct Schedule {
    systems: Vec<Entry>,
    /// Indices into `systems`, in execution order. Empty when `systems` is empty.
    order: Vec<usize>,
}

impl Schedule {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a system with no ordering constraints.
    pub fn add_system(
        &mut self,
        name: &'static str,
        system: impl FnMut(&mut World) + 'static,
    ) -> Result<&mut Self, ScheduleError> {
        self.insert(name, system, None)
    }

    /// Appends a system that runs before `before`. `before` must already be registered.
    pub fn add_system_before(
        &mut self,
        name: &'static str,
        before: &'static str,
        system: impl FnMut(&mut World) + 'static,
    ) -> Result<&mut Self, ScheduleError> {
        self.insert(name, system, Some(Constraint::Before(before)))
    }

    /// Appends a system that runs after `after`. `after` must already be registered.
    pub fn add_system_after(
        &mut self,
        name: &'static str,
        after: &'static str,
        system: impl FnMut(&mut World) + 'static,
    ) -> Result<&mut Self, ScheduleError> {
        self.insert(name, system, Some(Constraint::After(after)))
    }

    /// Requires `system` to run before `before`. Both names must already be registered.
    /// Saying the same thing twice is a no-op.
    pub fn before(&mut self, system: &'static str, before: &'static str) -> Result<(), ScheduleError> {
        self.constrain(system, Constraint::Before(before))
    }

    /// Requires `system` to run after `after`. Both names must already be registered.
    /// Saying the same thing twice is a no-op.
    pub fn after(&mut self, system: &'static str, after: &'static str) -> Result<(), ScheduleError> {
        self.constrain(system, Constraint::After(after))
    }

    /// Names in registration order.
    pub fn system_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.systems.iter().map(|s| s.name)
    }

    /// Names in the order [`Schedule::run`] uses.
    pub fn execution_order(&self) -> Vec<&'static str> {
        self.order.iter().map(|&i| self.systems[i].name).collect()
    }

    pub fn run(&mut self, world: &mut World) {
        for index in self.order.clone() {
            (self.systems[index].system)(world);
        }
    }

    fn insert(
        &mut self,
        name: &'static str,
        system: impl FnMut(&mut World) + 'static,
        constraint: Option<Constraint>,
    ) -> Result<&mut Self, ScheduleError> {
        if self.index_of(name).is_some() {
            return Err(ScheduleError::DuplicateName(name));
        }
        let (before, after) = match constraint {
            Some(c) => self.lists_for_new(name, c)?,
            None => (Vec::new(), Vec::new()),
        };
        self.systems.push(Entry { name, system: Box::new(system), before, after });
        match self.compute_order() {
            Ok(order) => {
                self.order = order;
                Ok(self)
            }
            Err(e) => {
                self.systems.pop();
                Err(e)
            }
        }
    }

    fn lists_for_new(
        &self,
        name: &'static str,
        constraint: Constraint,
    ) -> Result<(Vec<&'static str>, Vec<&'static str>), ScheduleError> {
        let target = constraint.target();
        if target == name {
            return Err(ScheduleError::SelfDependency(name));
        }
        if self.index_of(target).is_none() {
            return Err(ScheduleError::MissingTarget { system: name, target });
        }
        Ok(match constraint {
            Constraint::Before(_) => (vec![target], Vec::new()),
            Constraint::After(_) => (Vec::new(), vec![target]),
        })
    }

    fn constrain(&mut self, system: &'static str, constraint: Constraint) -> Result<(), ScheduleError> {
        let target = constraint.target();
        if target == system {
            return Err(ScheduleError::SelfDependency(system));
        }
        let Some(index) = self.index_of(system) else {
            return Err(ScheduleError::UnknownSystem(system));
        };
        if self.index_of(target).is_none() {
            return Err(ScheduleError::MissingTarget { system, target });
        }
        {
            let list = match constraint {
                Constraint::Before(_) => &mut self.systems[index].before,
                Constraint::After(_) => &mut self.systems[index].after,
            };
            if list.contains(&target) {
                return Ok(());
            }
            list.push(target);
        }
        match self.compute_order() {
            Ok(order) => {
                self.order = order;
                Ok(())
            }
            Err(e) => {
                match constraint {
                    Constraint::Before(_) => self.systems[index].before.pop(),
                    Constraint::After(_) => self.systems[index].after.pop(),
                };
                Err(e)
            }
        }
    }

    fn index_of(&self, name: &'static str) -> Option<usize> {
        self.systems.iter().position(|s| s.name == name)
    }

    /// Kahn's algorithm. Ready systems are taken in registration order (smallest index).
    fn compute_order(&self) -> Result<Vec<usize>, ScheduleError> {
        let n = self.systems.len();
        let mut by_name: HashMap<&'static str, usize> = HashMap::with_capacity(n);
        for (i, system) in self.systems.iter().enumerate() {
            by_name.insert(system.name, i);
        }

        let mut outgoing = vec![Vec::<usize>::new(); n];
        let mut indegree = vec![0u32; n];
        let mut seen = HashSet::new();
        for (i, system) in self.systems.iter().enumerate() {
            for &target in &system.before {
                let j = *by_name.get(target).ok_or(ScheduleError::MissingTarget { system: system.name, target })?;
                if i == j {
                    return Err(ScheduleError::SelfDependency(system.name));
                }
                // `i` runs before `j`.
                if seen.insert((i, j)) {
                    outgoing[i].push(j);
                    indegree[j] += 1;
                }
            }
            for &target in &system.after {
                let j = *by_name.get(target).ok_or(ScheduleError::MissingTarget { system: system.name, target })?;
                if i == j {
                    return Err(ScheduleError::SelfDependency(system.name));
                }
                // `j` runs before `i`.
                if seen.insert((j, i)) {
                    outgoing[j].push(i);
                    indegree[i] += 1;
                }
            }
        }

        let mut ready: BTreeSet<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
        let mut order = Vec::with_capacity(n);
        while let Some(i) = ready.pop_first() {
            order.push(i);
            for &j in &outgoing[i] {
                indegree[j] -= 1;
                if indegree[j] == 0 {
                    ready.insert(j);
                }
            }
        }
        if order.len() == n {
            Ok(order)
        } else {
            let involved = (0..n).filter(|i| !order.contains(i)).map(|i| self.systems[i].name).collect();
            Err(ScheduleError::Cycle(involved))
        }
    }
}

#[derive(Clone, Copy)]
enum Constraint {
    Before(&'static str),
    After(&'static str),
}

impl Constraint {
    fn target(self) -> &'static str {
        match self {
            Constraint::Before(name) | Constraint::After(name) => name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push(label: &'static str) -> impl FnMut(&mut World) {
        move |world| world.resource_mut::<Vec<&'static str>>().push(label)
    }

    fn labels(world: &World) -> Vec<&'static str> {
        world.resource::<Vec<&str>>().clone()
    }

    fn err<T, E>(result: Result<T, E>) -> E {
        match result {
            Ok(_) => panic!("expected an error"),
            Err(error) => error,
        }
    }

    #[test]
    fn systems_run_in_insertion_order() {
        let mut world = World::new();
        world.insert_resource(Vec::<&'static str>::new());
        let mut schedule = Schedule::new();
        schedule.add_system("a", push("a")).unwrap().add_system("b", push("b")).unwrap();
        schedule.run(&mut world);
        schedule.run(&mut world);
        assert_eq!(labels(&world), ["a", "b", "a", "b"]);
        assert_eq!(schedule.system_names().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(schedule.execution_order(), ["a", "b"]);
    }

    #[test]
    fn a_later_registration_runs_first_when_it_is_constrained_before() {
        let mut schedule = Schedule::new();
        schedule.add_system("first", push("first")).unwrap();
        schedule.add_system_before("second", "first", push("second")).unwrap();
        assert_eq!(schedule.execution_order(), ["second", "first"]);
        assert_eq!(schedule.system_names().collect::<Vec<_>>(), ["first", "second"]);
    }

    #[test]
    fn after_is_the_same_edge_as_before() {
        let mut schedule = Schedule::new();
        schedule.add_system("a", push("a")).unwrap();
        schedule.add_system("b", push("b")).unwrap();
        schedule.after("a", "b").unwrap();
        assert_eq!(schedule.execution_order(), ["b", "a"]);
        // The same precedence stored from the other system is not a second edge.
        schedule.before("b", "a").unwrap();
        assert_eq!(schedule.execution_order(), ["b", "a"]);
    }

    #[test]
    fn ready_systems_break_ties_by_registration_order() {
        // C must run before A. B has no constraint. B and C are both ready at the start,
        // and B was registered earlier, so the only order this schedule produces is B, C, A.
        // C, B, A and C, A, B would also satisfy the constraint; they are not chosen.
        let mut world = World::new();
        world.insert_resource(Vec::<&'static str>::new());
        let mut schedule = Schedule::new();
        schedule.add_system("a", push("a")).unwrap();
        schedule.add_system("b", push("b")).unwrap();
        schedule.add_system("c", push("c")).unwrap();
        schedule.before("c", "a").unwrap();
        assert_eq!(schedule.execution_order(), ["b", "c", "a"]);
        schedule.run(&mut world);
        assert_eq!(labels(&world), ["b", "c", "a"]);
    }

    #[test]
    fn a_chain_of_constraints_beats_registration_order() {
        let mut schedule = Schedule::new();
        schedule.add_system("a", push("a")).unwrap();
        schedule.add_system("b", push("b")).unwrap();
        schedule.add_system("c", push("c")).unwrap();
        schedule.before("a", "b").unwrap();
        schedule.after("b", "c").unwrap();
        assert_eq!(schedule.execution_order(), ["a", "c", "b"]);
    }

    #[test]
    fn duplicate_name_keeps_the_first_system() {
        let mut world = World::new();
        world.insert_resource(0u32);
        let mut schedule = Schedule::new();
        schedule.add_system("a", |w| *w.resource_mut::<u32>() += 1).unwrap();
        let err = err(schedule.add_system("a", |w| *w.resource_mut::<u32>() += 10));
        assert_eq!(err, ScheduleError::DuplicateName("a"));
        assert!(err.to_string().contains("`a`"));
        schedule.run(&mut world);
        assert_eq!(*world.resource::<u32>(), 1);
    }

    #[test]
    fn missing_and_unknown_targets_are_errors() {
        let mut schedule = Schedule::new();
        schedule.add_system("a", push("a")).unwrap();
        let missing = err(schedule.before("a", "missing"));
        assert_eq!(missing, ScheduleError::MissingTarget { system: "a", target: "missing" });
        let unknown = err(schedule.before("ghost", "a"));
        assert_eq!(unknown, ScheduleError::UnknownSystem("ghost"));
        let added = err(schedule.add_system_after("b", "nobody", push("b")));
        assert_eq!(added, ScheduleError::MissingTarget { system: "b", target: "nobody" });
        assert_eq!(schedule.execution_order(), ["a"]);
    }

    #[test]
    fn a_system_cannot_depend_on_itself() {
        let mut schedule = Schedule::new();
        let failed = err(schedule.add_system_before("a", "a", push("a")));
        assert_eq!(failed, ScheduleError::SelfDependency("a"));
        assert!(schedule.system_names().next().is_none());
        schedule.add_system("a", push("a")).unwrap();
        assert_eq!(err(schedule.after("a", "a")), ScheduleError::SelfDependency("a"));
        assert_eq!(schedule.execution_order(), ["a"]);
    }

    #[test]
    fn a_cycle_is_rejected_and_run_keeps_the_previous_order() {
        let mut world = World::new();
        world.insert_resource(Vec::<&'static str>::new());
        let mut schedule = Schedule::new();
        schedule.add_system("a", push("a")).unwrap();
        schedule.add_system("b", push("b")).unwrap();
        schedule.add_system("c", push("c")).unwrap();
        schedule.before("a", "b").unwrap();
        schedule.before("b", "c").unwrap();
        assert_eq!(schedule.execution_order(), ["a", "b", "c"]);
        let err = err(schedule.before("c", "a"));
        assert_eq!(err, ScheduleError::Cycle(vec!["a", "b", "c"]));
        assert!(err.to_string().contains("cycle"));
        schedule.run(&mut world);
        assert_eq!(labels(&world), ["a", "b", "c"]);
    }

    #[test]
    fn repeating_a_constraint_does_not_change_the_order() {
        let mut schedule = Schedule::new();
        schedule.add_system("a", push("a")).unwrap();
        schedule.add_system("b", push("b")).unwrap();
        schedule.before("a", "b").unwrap();
        schedule.before("a", "b").unwrap();
        assert_eq!(schedule.execution_order(), ["a", "b"]);
    }
}
