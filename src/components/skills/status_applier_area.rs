use crate::components::char::CharacterStateComponent;
use crate::components::skills::skill::{SkillManifestation, WorldCollisions};
use crate::components::status::{ApplyStatusComponent, ApplyStatusComponentPayload};
use crate::systems::render::render_command::RenderCommandCollectorComponent;
use crate::systems::{AssetResources, Collision, SystemVariables};
use crate::{ElapsedTime, PhysicEngine};
use nalgebra::Vector2;
use ncollide2d::pipeline::CollisionGroups;
use ncollide2d::shape::ShapeHandle;
use nphysics2d::object::{ColliderDesc, ColliderHandle, DefaultColliderHandle};
use specs::{Entity, LazyUpdate};
use std::collections::HashMap;

pub struct StatusApplierArea<F>
where
    F: FnMut(ElapsedTime) -> ApplyStatusComponentPayload,
{
    pub collider_handle: DefaultColliderHandle,
    pub extents: Vector2<f32>,
    pub pos: Vector2<f32>,
    pub name: &'static str,
    pub status_creator: F,
    pub caster_entity_id: Entity,
    pub next_action_at: ElapsedTime,
}

impl<F> StatusApplierArea<F>
where
    F: FnMut(ElapsedTime) -> ApplyStatusComponentPayload,
{
    pub fn new(
        name: &'static str,
        status_creator: F,
        skill_center: &Vector2<f32>,
        size: Vector2<f32>,
        caster_entity_id: Entity,
        physics_world: &mut PhysicEngine,
    ) -> StatusApplierArea<F> {
        let collider_handle = physics_world.add_cuboid_skill(*skill_center, 0.0, size);
        StatusApplierArea {
            collider_handle,
            name,
            status_creator,
            pos: *skill_center,
            extents: size,
            caster_entity_id,
            next_action_at: ElapsedTime(0.0),
        }
    }
}

impl<F> SkillManifestation for StatusApplierArea<F>
where
    F: FnMut(ElapsedTime) -> ApplyStatusComponentPayload,
{
    fn update(
        &mut self,
        _self_entity_id: Entity,
        all_collisions_in_world: &WorldCollisions,
        system_vars: &mut SystemVariables,
        _entities: &specs::Entities,
        _char_storage: &specs::ReadStorage<CharacterStateComponent>,
        physics_world: &mut PhysicEngine,
        _updater: &mut specs::Write<LazyUpdate>,
    ) {
        if self.next_action_at.is_earlier_than(system_vars.time) {
            let self_collider_handle = self.collider_handle;
            let my_collisions = all_collisions_in_world
                .iter()
                .filter(|(_key, coll)| coll.other_coll_handle == self_collider_handle);
            for (_key, coll) in my_collisions {
                let char_collider = physics_world
                    .colliders
                    .get(coll.character_coll_handle)
                    .unwrap();
                let char_entity_id = *char_collider
                    .user_data()
                    .map(|v| v.downcast_ref().unwrap())
                    .unwrap();
                system_vars.apply_statuses.push(ApplyStatusComponent {
                    source_entity_id: self.caster_entity_id,
                    target_entity_id: char_entity_id,
                    status: (self.status_creator)(system_vars.time),
                });
                self.next_action_at = system_vars.time.add_seconds(2.0);
            }
        }
    }

    fn render(
        &self,
        now: ElapsedTime,
        assets: &AssetResources,
        render_commands: &mut RenderCommandCollectorComponent,
    ) {
        render_commands
            .prepare_for_3d()
            .pos_2d(&self.pos)
            .color(&[0.0, 1.0, 0.0, 1.0])
            .add_rectangle_command(&(self.extents));

        render_commands
            .prepare_for_3d()
            .pos_2d(&self.pos)
            .y(3.0)
            .color(&if self.next_action_at.is_earlier_than(now) {
                [0.0, 1.0, 0.0, 1.0]
            } else {
                [0.3, 0.3, 0.3, 1.0]
            })
            .add_billboard_command(&assets.texts.custom_texts[self.name], false);
    }
}
