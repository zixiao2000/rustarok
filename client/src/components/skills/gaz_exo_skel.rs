use crate::components::char::ActionPlayMode;
use crate::components::char::CharacterStateComponent;
use crate::components::skills::skills::{
    FinishCast, SkillDef, SkillManifestation, SkillTargetType,
};
use crate::components::status::status::{
    ApplyStatusComponent, StatusEnum, StatusUpdateParams, StatusUpdateResult,
};
use crate::components::StrEffectComponent;
use crate::effect::StrEffectType;
use crate::systems::SystemVariables;
use rustarok_common::attack::BasicAttackType;
use rustarok_common::char_attr::{CharAttributeModifier, CharAttributeModifierCollector};
use rustarok_common::common::{EngineTime, LocalTime, Percentage};
use rustarok_common::config::CommonConfigs;
use specs::world::WorldExt;
use specs::{Entities, LazyUpdate};

pub struct ExoSkeletonSkill;

pub const EXO_SKELETON_SKILL: &'static ExoSkeletonSkill = &ExoSkeletonSkill;

impl SkillDef for ExoSkeletonSkill {
    fn get_icon_path(&self) -> &'static str {
        "data\\texture\\À¯ÀúÀÎÅÍÆäÀÌ½º\\item\\cr_reflectshield.bmp"
    }

    fn finish_cast(
        &self,
        params: &FinishCast,
        ecs_world: &mut specs::world::World,
    ) -> Option<Box<dyn SkillManifestation>> {
        let mut sys_vars = ecs_world.write_resource::<SystemVariables>();
        let now = ecs_world.read_resource::<EngineTime>().now();
        let configs = &ecs_world
            .read_resource::<CommonConfigs>()
            .skills
            .exoskeleton;
        let duration_seconds = configs.duration_seconds;
        sys_vars
            .apply_statuses
            .push(ApplyStatusComponent::from_status(
                params.caster_entity_id,
                params.caster_entity_id,
                StatusEnum::ExoSkeletonStatus(ExoSkeletonStatus::new(
                    now,
                    duration_seconds,
                    configs.armor,
                    configs.attack_range,
                    configs.movement_speed,
                    configs.attack_damage,
                    configs.attack_speed,
                )),
            ));
        None
    }

    fn get_skill_target_type(&self) -> SkillTargetType {
        SkillTargetType::NoTarget
    }
}

#[derive(Clone, Debug)]
pub struct ExoSkeletonStatus {
    started: LocalTime,
    pub until: LocalTime,
    armor: Percentage,
    attack_range: Percentage,
    movement_speed: Percentage,
    attack_damage: Percentage,
    attack_speed: Percentage,
}

impl ExoSkeletonStatus {
    fn new(
        now: LocalTime,
        duration: f32,
        armor: Percentage,
        attack_range: Percentage,
        movement_speed: Percentage,
        attack_damage: Percentage,
        attack_speed: Percentage,
    ) -> ExoSkeletonStatus {
        ExoSkeletonStatus {
            started: now,
            until: now.add_seconds(duration),
            armor,
            attack_range,
            movement_speed,
            attack_damage,
            attack_speed,
        }
    }
}

impl ExoSkeletonStatus {
    pub fn on_apply(
        &mut self,
        target_char: &mut CharacterStateComponent,
        entities: &Entities,
        updater: &mut LazyUpdate,
        now: LocalTime,
    ) {
        // TODO2
        //        target_char.basic_attack_type = BasicAttackType::Ranged {
        //            bullet_type: WeaponType::SilverBullet,
        //        };
        //        updater.insert(
        //            entities.create(),
        //            StrEffectComponent {
        //                effect_id: StrEffectType::Cart.into(),
        //                pos: target_char.pos(),
        //                start_time: now,
        //                die_at: None,
        //                play_mode: ActionPlayMode::Once,
        //            },
        //        );
    }

    pub fn calc_attribs(&self, modifiers: &mut CharAttributeModifierCollector) {
        modifiers.change_armor(
            CharAttributeModifier::AddPercentage(self.armor),
            self.started,
            self.until,
        );
        modifiers.change_walking_speed(
            CharAttributeModifier::AddPercentage(self.movement_speed),
            self.started,
            self.until,
        );
        modifiers.change_attack_range(
            CharAttributeModifier::AddPercentage(self.attack_range),
            self.started,
            self.until,
        );
        modifiers.change_attack_damage(
            CharAttributeModifier::IncreaseByPercentage(self.attack_damage),
            self.started,
            self.until,
        );
        modifiers.change_attack_speed(
            CharAttributeModifier::AddPercentage(self.attack_speed),
            self.started,
            self.until,
        );
    }

    pub fn update(&mut self, params: StatusUpdateParams) -> StatusUpdateResult {
        if self.until.has_already_passed(params.time.now()) {
            params.target_char.basic_attack_type = BasicAttackType::MeleeSimple;
            StatusUpdateResult::RemoveIt
        } else {
            StatusUpdateResult::KeepIt
        }
    }

    pub fn get_status_completion_percent(&self, now: LocalTime) -> Option<(LocalTime, f32)> {
        Some((self.until, now.percentage_between(self.started, self.until)))
    }
}
