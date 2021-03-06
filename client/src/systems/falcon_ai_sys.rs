use crate::components::char::{
    CharActionIndex, CharacterStateComponent, SpriteRenderDescriptorComponent,
};
use crate::components::controller::LocalPlayerController;
use crate::components::skills::falcon_carry::FalconCarryStatus;
use crate::components::status::status::{ApplyStatusComponent, StatusEnum};
use crate::runtime_assets::map::PhysicEngine;
use crate::systems::{SystemFrameDurations, SystemVariables};
use nalgebra::{Isometry2, Vector2, Vector3};
use rustarok_common::common::{v2, v2_to_v3, v3, v3_to_v2, EngineTime, LocalTime, Vec2};
use rustarok_common::components::char::{
    CharDir, ControllerEntityId, LocalCharEntityId, LocalCharStateComp,
};
use rustarok_common::components::controller::{ControllerComponent, PlayerIntention};
use specs::prelude::*;
use vek::QuadraticBezier3;

pub struct FalconAiSystem;

pub enum FalconState {
    Follow,
    Attack {
        started_at: LocalTime,
        ends_at: LocalTime,
        start_pos: Vector3<f32>,
        end_pos: Vector3<f32>,
    },
    CarryOwner {
        owner_controller_id: ControllerEntityId,
        started_at: LocalTime,
        ends_at: LocalTime,
        target_is_caught: bool,
        start_pos: Vector3<f32>,
    },
    CarryAlly {
        target_id: LocalCharEntityId,
        start_pos: Vector3<f32>,
        started_at: LocalTime,
        ends_at: LocalTime,
        target_is_caught: bool,
        end_pos: Vec2,
    },
}

#[derive(Component)]
pub struct FalconComponent {
    pub owner_entity_id: LocalCharEntityId,
    state: FalconState,
    pub pos: Vector3<f32>,
    acceleration: f32,
    bezier: QuadraticBezier3<f32>,
}

impl FalconComponent {
    pub fn new(owner_entity_id: LocalCharEntityId, start_x: f32, start_y: f32) -> FalconComponent {
        FalconComponent {
            owner_entity_id,
            state: FalconState::Follow,
            pos: Vector3::new(start_x, FALCON_FLY_HEIGHT, start_y),
            acceleration: 0.0,
            bezier: QuadraticBezier3 {
                start: vek::Vec3::new(0.0, 0.0, 0.0),
                ctrl: vek::Vec3::new(0.0, 0.0, 0.0),
                end: vek::Vec3::new(0.0, 0.0, 0.0),
            },
        }
    }

    pub fn carry_owner(
        &mut self,
        owner_controller_id: ControllerEntityId,
        target_pos: &Vec2,
        now: LocalTime,
        duration: f32,
        falcon_sprite: &mut SpriteRenderDescriptorComponent,
    ) {
        match self.state {
            FalconState::Follow { .. } => {}
            _ => return,
        }
        self.state = FalconState::CarryOwner {
            owner_controller_id,
            started_at: now,
            ends_at: now.add_seconds(duration),
            target_is_caught: false,
            start_pos: self.pos,
        };
        falcon_sprite.action_index = CharActionIndex::Walking as usize;
        falcon_sprite.direction = CharDir::determine_dir(&target_pos, &v3_to_v2(&self.pos));
    }

    pub fn carry_ally(
        &mut self,
        target_entity: LocalCharEntityId,
        target_pos: &Vec2,
        now: LocalTime,
        duration: f32,
        falcon_sprite: &mut SpriteRenderDescriptorComponent,
    ) {
        match self.state {
            FalconState::Follow { .. } => {}
            _ => return,
        }
        self.state = FalconState::CarryAlly {
            target_id: target_entity,
            start_pos: self.pos,
            started_at: now,
            ends_at: now.add_seconds(duration),
            target_is_caught: false,
            end_pos: Vector2::zeros(),
        };
        falcon_sprite.action_index = CharActionIndex::Walking as usize;
        falcon_sprite.direction = CharDir::determine_dir(&target_pos, &v3_to_v2(&self.pos));
    }

    pub fn set_state_to_attack(
        &mut self,
        now: LocalTime,
        duration: f32,
        start_pos: Vec2,
        end_pos: Vec2,
        falcon_sprite: &mut SpriteRenderDescriptorComponent,
    ) {
        match self.state {
            FalconState::Follow { .. } => {}
            _ => return,
        }
        self.pos = Vector3::new(start_pos.x, FALCON_FLY_HEIGHT, start_pos.y);
        self.state = FalconState::Attack {
            started_at: now,
            ends_at: now.add_seconds(duration),
            start_pos: self.pos,
            end_pos: Vector3::new(end_pos.x, 0.5, end_pos.y),
        };
        falcon_sprite.action_index = CharActionIndex::Walking as usize;
        falcon_sprite.direction = CharDir::determine_dir(&end_pos, &start_pos);
    }
}

impl FalconAiSystem {}

pub const FALCON_FLY_HEIGHT: f32 = 5.0;
pub const FALCON_LOWERED_HEIGHT: f32 = 2.0;

impl<'a> System<'a> for FalconAiSystem {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, FalconComponent>,
        WriteStorage<'a, SpriteRenderDescriptorComponent>,
        ReadStorage<'a, LocalCharStateComp>,
        ReadStorage<'a, ControllerComponent>,
        ReadExpect<'a, LocalPlayerController>,
        WriteExpect<'a, SystemFrameDurations>,
        WriteExpect<'a, PhysicEngine>,
        WriteExpect<'a, SystemVariables>,
        ReadExpect<'a, EngineTime>,
    );

    fn run(
        &mut self,
        (
            entities,
            mut falcon_storage,
            mut sprite_storage,
            auth_char_storage,
            controller_storage,
            local_player,
            mut system_benchmark,
            mut physics_world,
            mut sys_vars,
            time,
        ): Self::SystemData,
    ) {
        let _stopwatch = system_benchmark.start_measurement("FalconAiSystem");
        for (falcon_id, falcon, sprite) in
            (&entities, &mut falcon_storage, &mut sprite_storage).join()
        {
            match falcon.state {
                FalconState::Follow => {
                    if let Some(auth_owner) = auth_char_storage.get(falcon.owner_entity_id.into()) {
                        let falcon_pos_2d = v3_to_v2(&falcon.pos);
                        let diff_v = auth_owner.pos() - falcon_pos_2d;
                        let distance = diff_v.magnitude();
                        if distance > 2.0 {
                            let dir_3d = Vector3::new(
                                auth_owner.pos().x,
                                FALCON_FLY_HEIGHT,
                                auth_owner.pos().y,
                            ) - falcon.pos;
                        // TODO falcon solve it without dt(), or get dt from elsewhere not enginetime
                        //                            falcon.acceleration = (falcon.acceleration + time.dt() * 0.05).min(
                        //                                0.03 * auth_owner.calculated_attribs().movement_speed.as_f32(),
                        //                            );
                        //                            falcon.pos += dir_3d * falcon.acceleration;
                        //                            sprite.direction =
                        //                                CharDir::determine_dir(&auth_owner.pos(), &falcon_pos_2d);
                        } else {
                            if falcon.acceleration < 0.00001 || distance == 0.0 {
                                falcon.acceleration = 0.0;
                            } else {
                                // TODO falcon solve it without dt(), or get dt from elsewhere not enginetime
                                //                                falcon.acceleration -= time.dt() * 0.1;
                                let dir = diff_v.normalize();
                                falcon.pos += v2_to_v3(&dir) * falcon.acceleration;
                            }
                        }
                    } else {
                        entities.delete(falcon_id).expect("");
                    }
                }
                FalconState::CarryOwner {
                    owner_controller_id,
                    started_at,
                    ends_at,
                    target_is_caught,
                    start_pos,
                } => {
                    let duration_percentage = time.now().percentage_between(started_at, ends_at);

                    let pick_duration =
                        (time.now().as_seconds_f32() - started_at.as_seconds_f32()) / 0.3;
                    if pick_duration <= 1.0 {
                        if let Some(target) = auth_char_storage.get(falcon.owner_entity_id.into()) {
                            let target_pos = target.pos();
                            let line =
                                Vector3::new(target_pos.x, FALCON_LOWERED_HEIGHT, target_pos.y)
                                    - start_pos;
                            falcon.pos = start_pos + line * pick_duration;
                        } else {
                            entities.delete(falcon_id).expect("");
                            return;
                        }
                    } else if duration_percentage < 1.0 {
                        if !target_is_caught {
                            sprite.action_index = CharActionIndex::Idle as usize;
                            sys_vars
                                .apply_statuses
                                .push(ApplyStatusComponent::from_status(
                                    falcon.owner_entity_id,
                                    falcon.owner_entity_id,
                                    StatusEnum::FalconCarryStatus(FalconCarryStatus {
                                        started_at,
                                        ends_at,
                                        carry_owner: true,
                                        end_pos: Vector2::zeros(),
                                    }),
                                ));
                            falcon.state = FalconState::CarryOwner {
                                owner_controller_id,
                                started_at,
                                ends_at,
                                target_is_caught: true,
                                start_pos,
                            }
                        }
                        pub const FALCON_CARRY_HEIGHT: f32 = 12.0;
                        let y = if duration_percentage < 0.4 {
                            (duration_percentage / 0.4) * (FALCON_CARRY_HEIGHT - 2.0) + 2.0
                        } else {
                            (1.0 - ((duration_percentage - 0.8).max(0.0) / 0.2))
                                * (FALCON_CARRY_HEIGHT - 2.0)
                                + 2.0
                        };
                        let target_pos = if let Some(controller) =
                            controller_storage.get(owner_controller_id.into())
                        {
                            match controller.intention {
                                Some(PlayerIntention::MoveTo(pos)) => v3(pos.x, y, pos.y),
                                Some(PlayerIntention::MoveTowardsMouse(pos)) => v3(pos.x, y, pos.y),
                                _ => {
                                    let last_intention = &local_player.last_intention;
                                    match last_intention {
                                        Some(PlayerIntention::MoveTo(pos)) => v3(pos.x, y, pos.y),
                                        Some(PlayerIntention::MoveTowardsMouse(pos)) => {
                                            v3(pos.x, y, pos.y)
                                        }
                                        _ => v3(falcon.pos.x, y, falcon.pos.z),
                                    }
                                }
                            }
                        } else {
                            entities.delete(falcon_id).expect("");
                            return;
                        };
                        falcon.pos.y = y;
                        let diff_v = target_pos - falcon.pos;
                        let distance = diff_v.magnitude();
                        if distance > 2.0 {
                            let falcon_pos_2d = v3_to_v2(&falcon.pos);
                            let dir_3d = (diff_v).normalize();
                            // TODO falcon solve it without dt(), or get dt from elsewhere not enginetime
                            //                            falcon.acceleration = 8.57 * time.dt();
                            falcon.pos += dir_3d * falcon.acceleration;
                            sprite.direction =
                                CharDir::determine_dir(&v3_to_v2(&target_pos), &falcon_pos_2d);
                        } else {
                            if falcon.acceleration < 0.00001 || distance == 0.0 {
                                falcon.acceleration = 0.0;
                            } else {
                                // TODO falcon solve it without dt(), or get dt from elsewhere not enginetime
                                //                                falcon.acceleration -= time.dt() * 0.1;
                                let dir = diff_v.normalize();
                                falcon.pos += dir * falcon.acceleration;
                            }
                        }

                        if let Some(target) = auth_char_storage.get(falcon.owner_entity_id.into()) {
                            // TODO4 falcon
                            //                            let body = physics_world
                            //                                .bodies
                            //                                .rigid_body_mut(target.body_handle)
                            //                                .unwrap();
                            //                            body.set_position(Isometry2::translation(falcon.pos.x, falcon.pos.z));
                            //                            target.set_y(falcon.pos.y - 2.5);
                        }
                    } else {
                        if let Some(target) = auth_char_storage.get(falcon.owner_entity_id.into()) {
                            // TODO4 falcon
                            //                            target.set_y(0.0);
                        }
                        falcon.state = FalconState::Follow;
                        sprite.action_index = CharActionIndex::Idle as usize;
                    }
                }
                FalconState::CarryAlly {
                    target_id,
                    start_pos,
                    started_at,
                    ends_at,
                    target_is_caught,
                    end_pos,
                } => {
                    let duration_percentage = time.now().percentage_between(started_at, ends_at);
                    // 30% of duration is to go for the ally
                    if duration_percentage <= 0.3 {
                        if let Some(target) = auth_char_storage.get(target_id.into()) {
                            let target_pos = target.pos();
                            let line =
                                Vector3::new(target_pos.x, FALCON_LOWERED_HEIGHT, target_pos.y)
                                    - start_pos;
                            let duration_percentage = duration_percentage / 0.3;
                            falcon.pos = start_pos + line * duration_percentage;
                        } else {
                            falcon.state = FalconState::Follow;
                            sprite.action_index = CharActionIndex::Idle as usize;
                        }
                    } else if duration_percentage < 1.0 {
                        if !target_is_caught {
                            if let Some(owner) =
                                auth_char_storage.get(falcon.owner_entity_id.into())
                            {
                                let line = v3_to_v2(&falcon.pos) - owner.pos();
                                let ctrl = v3_to_v2(&falcon.pos) + (line * 0.2) + v2(5.0, 0.0);
                                let ctrl = vek::Vec3::new(ctrl.x, 20.0, ctrl.y);
                                let end_pos = owner.pos();
                                sprite.action_index = CharActionIndex::Idle as usize;
                                falcon.bezier = QuadraticBezier3 {
                                    start: vek::Vec3::new(falcon.pos.x, falcon.pos.y, falcon.pos.z),
                                    ctrl,
                                    end: vek::Vec3::new(
                                        end_pos.x,
                                        FALCON_LOWERED_HEIGHT,
                                        end_pos.y,
                                    ),
                                };
                                sys_vars
                                    .apply_statuses
                                    .push(ApplyStatusComponent::from_status(
                                        falcon.owner_entity_id,
                                        target_id,
                                        StatusEnum::FalconCarryStatus(FalconCarryStatus {
                                            started_at,
                                            ends_at,
                                            carry_owner: false,
                                            end_pos,
                                        }),
                                    ));
                                falcon.state = FalconState::CarryAlly {
                                    target_id,
                                    start_pos,
                                    started_at,
                                    ends_at,
                                    target_is_caught: true,
                                    end_pos,
                                }
                            } else {
                                entities.delete(falcon_id).expect("");
                                return;
                            }
                        }
                        let duration_percentage = (duration_percentage - 0.3) / 0.7;
                        let pos = falcon.bezier.evaluate(duration_percentage);
                        falcon.pos = v3(pos.x, pos.y, pos.z);
                        if let Some(target) = auth_char_storage.get(target_id.into()) {
                            // TODO4 falcon
                            //                            let body = physics_world
                            //                                .bodies
                            //                                .rigid_body_mut(target.body_handle)
                            //                                .unwrap();
                            //                            body.set_position(Isometry2::translation(falcon.pos.x, falcon.pos.z));
                            //                            target.set_y(falcon.pos.y - 2.5);
                        }
                        sprite.direction = CharDir::determine_dir(&end_pos, &v3_to_v2(&falcon.pos));
                    } else {
                        // TODO4 falcon
                        if let Some(target) = auth_char_storage.get(target_id.into()) {
                            //                            target.set_y(0.0);
                        }
                        falcon.state = FalconState::Follow;
                        sprite.action_index = CharActionIndex::Idle as usize;
                    }
                }
                FalconState::Attack {
                    started_at,
                    ends_at,
                    start_pos,
                    end_pos,
                } => {
                    let duration_percentage = time.now().percentage_between(started_at, ends_at);
                    if duration_percentage <= 1.0 {
                        let line = end_pos - start_pos;
                        falcon.pos = start_pos + line * duration_percentage;
                    } else {
                        falcon.state = FalconState::Follow;
                        sprite.action_index = CharActionIndex::Idle as usize;
                    }
                }
            }
        }
    }
}
