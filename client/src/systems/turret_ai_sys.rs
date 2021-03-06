use specs::prelude::*;

use crate::components::char::{
    CharacterStateComponent, TurretComponent, TurretControllerComponent,
};
use crate::systems::minion_ai_sys::MinionAiSystem;
use crate::systems::SystemFrameDurations;
use rustarok_common::common::v2_to_p2;
use rustarok_common::components::char::{
    ControllerEntityId, EntityTarget, LocalCharEntityId, LocalCharStateComp,
    StaticCharDataComponent,
};
use rustarok_common::components::controller::{ControllerComponent, PlayerIntention};
use rustarok_common::config::CommonConfigs;

pub struct TurretAiSystem;

impl TurretAiSystem {}

impl<'a> System<'a> for TurretAiSystem {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, ControllerComponent>,
        ReadStorage<'a, CharacterStateComponent>,
        ReadStorage<'a, StaticCharDataComponent>,
        ReadStorage<'a, LocalCharStateComp>,
        ReadStorage<'a, TurretControllerComponent>,
        ReadStorage<'a, TurretComponent>,
        WriteExpect<'a, SystemFrameDurations>,
        ReadExpect<'a, CommonConfigs>,
    );

    fn run(
        &mut self,
        (
            entities,
            mut controller_storage,
            char_state_storage,
            static_char_data_storage,
            auth_char_state_storage,
            turret_controller_storage,
            turret_storage,
            mut system_benchmark,
            dev_configs,
        ): Self::SystemData,
    ) {
        let _stopwatch = system_benchmark.start_measurement("TurretAiSystem");
        for (controller_id, controller, _turret) in (
            &entities,
            &mut controller_storage,
            &turret_controller_storage,
        )
            .join()
        {
            let controller_id = ControllerEntityId::new(controller_id);
            let radius = dev_configs.skills.gaz_turret.turret.attack_range.as_f32() * 100.0;
            let controlled_entity_id = controller.controlled_entity.unwrap();
            let char_state = auth_char_state_storage.get(controlled_entity_id.into());

            if let Some(char_state) = char_state {
                // at this point, preferred target is an enemy for sure
                let preferred_target_id = turret_storage
                    .get(controlled_entity_id.into())
                    .unwrap()
                    .preferred_target;
                if let Some(preferred_target_id) = preferred_target_id {
                    if char_state
                        .target
                        .as_ref()
                        .map(|it| match it {
                            EntityTarget::OtherEntity(target_id) => {
                                *target_id != preferred_target_id
                            }
                            _ => true,
                        })
                        .unwrap_or(true)
                    {
                        if let Some(preferred_target) =
                            auth_char_state_storage.get(preferred_target_id.into())
                        {
                            let current_distance = nalgebra::distance(
                                &v2_to_p2(&preferred_target.pos()),
                                &v2_to_p2(&char_state.pos()),
                            );
                            if !preferred_target.state().is_dead() && current_distance < radius {
                                controller.intention =
                                    Some(PlayerIntention::Attack(preferred_target_id));
                                return;
                            }
                        }
                    }
                }
                // Hack
                let mut current_target_id = None;
                // hack end
                // first check if preferred target is in range

                let current_target_entity = match char_state.target {
                    Some(EntityTarget::OtherEntity(target_id)) => {
                        current_target_id = Some(target_id);
                        auth_char_state_storage.get(target_id.into())
                    }
                    _ => None,
                };
                let no_target_or_dead_or_out_of_range = match current_target_entity {
                    Some(target) => {
                        let current_distance = nalgebra::distance(
                            &v2_to_p2(&target.pos()),
                            &v2_to_p2(&char_state.pos()),
                        );
                        target.state().is_dead() || current_distance > radius
                    }
                    None => true,
                };

                let team = static_char_data_storage
                    .get(controlled_entity_id.into())
                    .unwrap()
                    .team;
                controller.intention = if no_target_or_dead_or_out_of_range {
                    let maybe_enemy = MinionAiSystem::get_closest_enemy_in_area(
                        &entities,
                        &static_char_data_storage,
                        &auth_char_state_storage,
                        &char_state.pos(),
                        radius,
                        team,
                        controlled_entity_id,
                    );
                    match maybe_enemy {
                        Some(target_id) => Some(PlayerIntention::Attack(target_id)),
                        None => None,
                    }
                } else {
                    Some(PlayerIntention::Attack(current_target_id.unwrap()))
                }
            } else {
                // the char might have died, remove the controller entity
                entities.delete(controller_id.into()).expect("");
            }
        }
    }
}
