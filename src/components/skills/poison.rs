use specs::LazyUpdate;

use crate::common::Vec2;
use crate::components::char::ActionPlayMode;
use crate::components::controller::CharEntityId;
use crate::components::skills::skills::{SkillDef, SkillManifestation, SkillTargetType};
use crate::components::status::status::{ApplyStatusComponent, PoisonStatus};
use crate::components::StrEffectComponent;
use crate::configs::DevConfig;
use crate::effect::StrEffectType;
use crate::systems::SystemVariables;

pub struct PosionSkill;

pub const POISON_SKILL: &'static PosionSkill = &PosionSkill;

impl SkillDef for PosionSkill {
    fn get_icon_path(&self) -> &'static str {
        "data\\texture\\À¯ÀúÀÎÅÍÆäÀÌ½º\\item\\tf_poison.bmp"
    }

    fn finish_cast(
        &self,
        caster_entity_id: CharEntityId,
        caster_pos: Vec2,
        skill_pos: Option<Vec2>,
        char_to_skill_dir: &Vec2,
        target_entity: Option<CharEntityId>,
        ecs_world: &mut specs::world::World,
    ) -> Option<Box<dyn SkillManifestation>> {
        let mut sys_vars = ecs_world.write_resource::<SystemVariables>();
        let entities = &ecs_world.entities();
        let updater = ecs_world.read_resource::<LazyUpdate>();
        let now = sys_vars.time;
        updater.insert(
            entities.create(),
            StrEffectComponent {
                effect_id: StrEffectType::Poison.into(),
                pos: skill_pos.unwrap(),
                start_time: now,
                die_at: Some(now.add_seconds(0.7)),
                play_mode: ActionPlayMode::Repeat,
            },
        );
        let configs = &ecs_world.read_resource::<DevConfig>().skills.poison;
        sys_vars
            .apply_statuses
            .push(ApplyStatusComponent::from_secondary_status(
                caster_entity_id,
                target_entity.unwrap(),
                Box::new(PoisonStatus {
                    poison_caster_entity_id: caster_entity_id,
                    started: now,
                    until: now.add_seconds(configs.duration_seconds),
                    next_damage_at: now,
                    damage: configs.damage,
                }),
            ));
        None
    }

    fn get_skill_target_type(&self) -> SkillTargetType {
        SkillTargetType::OnlyEnemy
    }
}
