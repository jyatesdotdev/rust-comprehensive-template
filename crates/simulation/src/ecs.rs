//! Minimal Entity Component System (ECS) pattern.
//!
//! Demonstrates the archetype-free, sparse-set style ECS using `TypeId` + `Any`.

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// An entity is just an opaque ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity(pub u64);

/// Sparse component storage: TypeId → (Entity → component).
type Storage = HashMap<TypeId, HashMap<u64, Box<dyn Any>>>;

/// A minimal ECS world.
pub struct World {
    next_id: u64,
    storage: Storage,
}

impl World {
    /// Create an empty world with no entities or components.
    pub fn new() -> Self {
        Self { next_id: 0, storage: HashMap::new() }
    }

    /// Spawn a new entity (no components yet).
    pub fn spawn(&mut self) -> Entity {
        let e = Entity(self.next_id);
        self.next_id += 1;
        e
    }

    /// Attach a component to an entity.
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        self.storage
            .entry(TypeId::of::<T>())
            .or_default()
            .insert(entity.0, Box::new(component));
    }

    /// Get an immutable reference to an entity's component.
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.storage
            .get(&TypeId::of::<T>())?
            .get(&entity.0)?
            .downcast_ref()
    }

    /// Get a mutable reference to an entity's component.
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.storage
            .get_mut(&TypeId::of::<T>())?
            .get_mut(&entity.0)?
            .downcast_mut()
    }

    /// Iterate all entities that have component `T`, yielding `(Entity, &T)`.
    pub fn query<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.storage
            .get(&TypeId::of::<T>())
            .into_iter()
            .flat_map(|map| {
                map.iter().filter_map(|(&id, val)| {
                    val.downcast_ref::<T>().map(|c| (Entity(id), c))
                })
            })
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

// ── Example components ──────────────────────────────────────────────

/// Example 2D position component.
#[derive(Debug, Clone, Copy)]
pub struct Position {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
}

/// Example 2D velocity component.
#[derive(Debug, Clone, Copy)]
pub struct Velocity {
    /// Velocity along the X axis.
    pub dx: f64,
    /// Velocity along the Y axis.
    pub dy: f64,
}

/// A "system" that moves all entities with both Position and Velocity.
pub fn movement_system(world: &mut World, dt: f64) {
    // Collect entities that have Velocity first (can't borrow world mutably while iterating).
    let updates: Vec<_> = world
        .query::<Velocity>()
        .map(|(e, &v)| (e, v))
        .collect();

    for (entity, vel) in updates {
        if let Some(pos) = world.get_mut::<Position>(entity) {
            pos.x += vel.dx * dt;
            pos.y += vel.dy * dt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_query() {
        let mut world = World::new();
        let e1 = world.spawn();
        let e2 = world.spawn();
        world.insert(e1, Position { x: 0.0, y: 0.0 });
        world.insert(e2, Position { x: 5.0, y: 5.0 });

        let positions: Vec<_> = world.query::<Position>().collect();
        assert_eq!(positions.len(), 2);
    }

    #[test]
    fn movement_system_updates_position() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position { x: 0.0, y: 0.0 });
        world.insert(e, Velocity { dx: 10.0, dy: -5.0 });

        movement_system(&mut world, 1.0);

        let pos = world.get::<Position>(e).unwrap();
        assert!((pos.x - 10.0).abs() < 1e-10);
        assert!((pos.y - -5.0).abs() < 1e-10);
    }

    #[test]
    fn entity_without_velocity_not_moved() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position { x: 1.0, y: 2.0 });

        movement_system(&mut world, 1.0);

        let pos = world.get::<Position>(e).unwrap();
        assert!((pos.x - 1.0).abs() < 1e-10);
    }
}
