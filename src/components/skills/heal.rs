use nalgebra::Vector2;
use specs::LazyUpdate;

use crate::components::controller::{CharEntityId, WorldCoord};
use crate::components::skills::skills::{SkillDef, SkillManifestation, SkillTargetType};

use crate::components::{AttackComponent, AttackType, SoundEffectComponent};
use crate::configs::DevConfig;
use crate::systems::SystemVariables;

pub struct HealSkill;

pub const HEAL_SKILL: &'static HealSkill = &HealSkill;

impl SkillDef for HealSkill {
    fn get_icon_path(&self) -> &'static str {
        "data\\texture\\À¯ÀúÀÎÅÍÆäÀÌ½º\\item\\al_heal.bmp"
    }

    fn finish_cast(
        &self,
        caster_entity_id: CharEntityId,
        caster_pos: WorldCoord,
        skill_pos: Option<Vector2<f32>>,
        char_to_skill_dir: &Vector2<f32>,
        target_entity: Option<CharEntityId>,
        ecs_world: &mut specs::world::World,
    ) -> Option<Box<dyn SkillManifestation>> {
        let target_entity_id = target_entity.unwrap();
        let entities = &ecs_world.entities();
        let updater = ecs_world.read_resource::<LazyUpdate>();
        let mut system_vars = ecs_world.write_resource::<SystemVariables>();
        let entity = entities.create();
        updater.insert(
            entity,
            SoundEffectComponent {
                target_entity_id,
                sound_id: system_vars.assets.sounds.heal,
                pos: caster_pos,
                start_time: system_vars.time,
            },
        );
        system_vars.attacks.push(AttackComponent {
            src_entity: caster_entity_id,
            dst_entity: target_entity_id,
            typ: AttackType::Heal(ecs_world.read_resource::<DevConfig>().skills.heal.heal),
        });
        None
    }

    fn get_skill_target_type(&self) -> SkillTargetType {
        SkillTargetType::OnlyAllyAndSelf
    }
}