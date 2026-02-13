// TODO: refactor

use crate::SkeletalComponent;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use syrillian::Reflect;
use syrillian::World;
use syrillian::components::Component;
use syrillian::core::GameObjectId;
use syrillian::math::{Quat, Vec3};
use syrillian::tracing::warn;
use syrillian::utils::animation::{
    AnimationClip, Binding, ChannelBinding, ClipIndex, sample_rotation, sample_scale,
    sample_translation,
};

const DEFAULT_CROSSFADE_DURATION: f32 = 0.2;
const LAYER_REMOVE_EPSILON: f32 = 1e-3;

#[derive(Debug, Clone)]
struct ActiveLayer {
    clip_index: usize,
    time: f32,
    speed: f32,
    looping: bool,
    weight: f32,
    target_weight: f32,
    fade_rate: f32,
}

impl ActiveLayer {
    fn new_immediate(
        clip_index: usize,
        looping: bool,
        speed: f32,
        start_weight: f32,
        target_weight: f32,
    ) -> Self {
        Self {
            clip_index,
            time: 0.0,
            speed,
            looping,
            weight: start_weight.clamp(0.0, 1.0),
            target_weight: target_weight.clamp(0.0, 1.0),
            fade_rate: 0.0,
        }
    }

    fn set_fade_target(&mut self, target_weight: f32, duration: f32) {
        self.target_weight = target_weight.clamp(0.0, 1.0);

        if duration <= 0.0 {
            self.weight = self.target_weight;
            self.fade_rate = 0.0;
            return;
        }

        self.fade_rate = (self.target_weight - self.weight) / duration;
    }

    fn step_weight(&mut self, dt: f32) {
        if self.fade_rate == 0.0 {
            return;
        }

        self.weight = (self.weight + self.fade_rate * dt).clamp(0.0, 1.0);

        let reached_target = (self.fade_rate > 0.0 && self.weight >= self.target_weight)
            || (self.fade_rate < 0.0 && self.weight <= self.target_weight);
        if reached_target {
            self.weight = self.target_weight;
            self.fade_rate = 0.0;
        }
    }
}

#[derive(Default, Reflect)]
pub struct AnimationComponent {
    // Multiple clips (by name)
    clips: Vec<AnimationClip>,
    clip_indices: Vec<ClipIndex>,

    // Active layered playback stack
    #[dont_reflect]
    layers: Vec<ActiveLayer>,

    bindings: Vec<Vec<ChannelBinding>>,
}

/// Position, Rotation, Scale
type SkeletonLocals = (Vec3, Quat, Vec3);

#[derive(Debug, Copy, Clone, Default)]
struct Vec3Accumulator {
    sum: Vec3,
    weight_sum: f32,
}

impl Vec3Accumulator {
    fn add(&mut self, value: Vec3, weight: f32) {
        if weight <= 0.0 {
            return;
        }
        self.sum += value * weight;
        self.weight_sum += weight;
    }

    fn mixed(&self) -> Option<Vec3> {
        (self.weight_sum > LAYER_REMOVE_EPSILON).then_some(self.sum / self.weight_sum)
    }

    fn blend_weight(&self) -> f32 {
        self.weight_sum.clamp(0.0, 1.0)
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct QuatAccumulator {
    sum_x: f32,
    sum_y: f32,
    sum_z: f32,
    sum_w: f32,
    weight_sum: f32,
    reference: Option<Quat>,
}

impl QuatAccumulator {
    fn add(&mut self, value: Quat, weight: f32) {
        if weight <= 0.0 {
            return;
        }

        let mut q = value.normalize();
        if let Some(reference) = self.reference {
            if reference.dot(q) < 0.0 {
                q = Quat::from_xyzw(-q.x, -q.y, -q.z, -q.w);
            }
        } else {
            self.reference = Some(q);
        }

        self.sum_x += q.x * weight;
        self.sum_y += q.y * weight;
        self.sum_z += q.z * weight;
        self.sum_w += q.w * weight;
        self.weight_sum += weight;
    }

    fn mixed(&self) -> Option<Quat> {
        if self.weight_sum <= LAYER_REMOVE_EPSILON {
            return None;
        }

        let inv = 1.0 / self.weight_sum;
        let q = Quat::from_xyzw(
            self.sum_x * inv,
            self.sum_y * inv,
            self.sum_z * inv,
            self.sum_w * inv,
        );

        if q.length_squared() <= f32::EPSILON {
            self.reference
        } else {
            Some(q.normalize())
        }
    }

    fn blend_weight(&self) -> f32 {
        self.weight_sum.clamp(0.0, 1.0)
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct PoseAccumulator {
    translation: Vec3Accumulator,
    rotation: QuatAccumulator,
    scale: Vec3Accumulator,
}

impl Component for AnimationComponent {
    fn update(&mut self, world: &mut World) {
        if self.layers.is_empty() || self.clips.is_empty() {
            return;
        }

        let dt = world.delta_time().as_secs_f32();
        self.advance_layers(dt);
        self.prune_layers();
        if self.layers.is_empty() {
            return;
        }

        self.evaluate_and_apply_layers();
    }
}

impl AnimationComponent {
    pub fn set_clips(&mut self, clips: Vec<AnimationClip>) {
        let clip_indices = clips.iter().map(ClipIndex::new).collect();
        self.clips = clips;
        self.clip_indices = clip_indices;
        self.resolve_bindings();
        self.layers.clear();
    }

    pub fn resolve_bindings(&mut self) {
        self.bindings.clear();
        self.bindings.reserve(self.clips.len());

        let mut map_nodes = HashMap::<String, GameObjectId>::new();
        collect_subtree_by_name(self.parent(), &mut map_nodes);

        let mut bone_map = HashMap::<String, Vec<(GameObjectId, usize)>>::new();
        let mut stack = vec![self.parent()];
        while let Some(go) = stack.pop() {
            if let Some(skel) = go.get_component::<SkeletalComponent>() {
                for (i, name) in skel.bones().names.iter().enumerate() {
                    match bone_map.get_mut(name) {
                        None => {
                            bone_map.insert(name.clone(), vec![(go, i)]);
                        }
                        Some(map) => {
                            map.push((go, i));
                        }
                    }
                }
            }
            for c in go.children().iter().copied() {
                stack.push(c);
            }
        }

        for clip in self.clips.iter() {
            let mut binds = Vec::<ChannelBinding>::with_capacity(clip.channels.len());
            for (ch_index, ch) in clip.channels.iter().enumerate() {
                if let Some(bones) = bone_map.get(&ch.target_name) {
                    for (skel_go, i) in bones.iter().copied() {
                        binds.push(ChannelBinding {
                            ch_index,
                            target: Binding::Bone {
                                skel: skel_go,
                                idx: i,
                            },
                        });
                    }
                } else if let Some(&go) = map_nodes.get(&ch.target_name) {
                    binds.push(ChannelBinding {
                        ch_index,
                        target: Binding::Transform(go),
                    });
                } else {
                    warn!(
                        "No valid animation binding found for channel {}",
                        ch.target_name
                    );
                }
            }
            self.bindings.push(binds);
        }
    }

    pub fn play_by_name(&mut self, name: &str, looping: bool, speed: f32, weight: f32) {
        let Some(index) = self.find_clip_index_by_name(name) else {
            warn!("No clip \"{name}\" found in {}", self.parent().name);
            return;
        };

        self.play_index(index, looping, speed, weight);
    }

    pub fn play_index(&mut self, index: usize, looping: bool, speed: f32, weight: f32) {
        if index >= self.clips.len() {
            warn!("No clip #{index} found in {}", self.parent().name);
            return;
        }

        let target_weight = weight.clamp(0.0, 1.0);
        self.layers.clear();
        if target_weight > 0.0 {
            self.layers.push(ActiveLayer::new_immediate(
                index,
                looping,
                speed,
                target_weight,
                target_weight,
            ));
        }
    }

    pub fn crossfade_by_name(
        &mut self,
        name: &str,
        duration: f32,
        looping: bool,
        speed: f32,
        target_weight: f32,
    ) -> bool {
        let Some(index) = self.find_clip_index_by_name(name) else {
            warn!("No clip \"{name}\" found in {}", self.parent().name);
            return false;
        };

        self.crossfade_index(index, duration, looping, speed, target_weight)
    }

    pub fn crossfade_index(
        &mut self,
        index: usize,
        duration: f32,
        looping: bool,
        speed: f32,
        target_weight: f32,
    ) -> bool {
        if index >= self.clips.len() {
            warn!("No clip #{index} found in {}", self.parent().name);
            return false;
        }

        let target_weight = target_weight.clamp(0.0, 1.0);
        if duration <= 0.0 {
            self.layers.clear();
            if target_weight > 0.0 {
                self.layers.push(ActiveLayer::new_immediate(
                    index,
                    looping,
                    speed,
                    target_weight,
                    target_weight,
                ));
            }
            return true;
        }

        for layer in &mut self.layers {
            layer.set_fade_target(0.0, duration);
        }

        if target_weight > 0.0 {
            let mut incoming = ActiveLayer::new_immediate(index, looping, speed, 0.0, 0.0);
            incoming.set_fade_target(target_weight, duration);
            self.layers.push(incoming);
        }

        self.prune_layers();
        true
    }

    pub fn crossfade_by_name_default(
        &mut self,
        name: &str,
        looping: bool,
        speed: f32,
        target_weight: f32,
    ) -> bool {
        self.crossfade_by_name(
            name,
            DEFAULT_CROSSFADE_DURATION,
            looping,
            speed,
            target_weight,
        )
    }

    pub fn crossfade_index_default(
        &mut self,
        index: usize,
        looping: bool,
        speed: f32,
        target_weight: f32,
    ) -> bool {
        self.crossfade_index(
            index,
            DEFAULT_CROSSFADE_DURATION,
            looping,
            speed,
            target_weight,
        )
    }

    fn find_clip_index_by_name(&self, name: &str) -> Option<usize> {
        self.clips.iter().position(|c| c.name == name)
    }

    fn advance_layers(&mut self, dt: f32) {
        for layer in &mut self.layers {
            layer.time += dt * layer.speed;
            let clip = &self.clips[layer.clip_index];
            if clip.duration > 0.0 {
                if layer.looping {
                    layer.time = layer.time.rem_euclid(clip.duration);
                } else if layer.time > clip.duration {
                    layer.time = clip.duration;
                } else if layer.time < 0.0 {
                    layer.time = 0.0;
                }
            } else {
                layer.time = 0.0;
            }

            layer.step_weight(dt);
        }
    }

    fn prune_layers(&mut self) {
        self.layers.retain(|layer| {
            layer.weight > LAYER_REMOVE_EPSILON || layer.target_weight > LAYER_REMOVE_EPSILON
        });
    }

    fn ensure_pose_accumulator(
        skel_go: GameObjectId,
        locals: &mut HashMap<GameObjectId, Vec<PoseAccumulator>>,
    ) -> Option<&mut Vec<PoseAccumulator>> {
        if !skel_go.exists() {
            return None;
        }

        match locals.entry(skel_go) {
            Entry::Occupied(o) => Some(o.into_mut()),
            Entry::Vacant(e) => {
                let skel = skel_go.get_component::<SkeletalComponent>()?;
                Some(e.insert(vec![PoseAccumulator::default(); skel.bone_count()]))
            }
        }
    }

    fn evaluate_and_apply_layers(&mut self) {
        let mut skel_locals: HashMap<GameObjectId, Vec<PoseAccumulator>> = HashMap::new();
        let mut transform_locals: HashMap<GameObjectId, PoseAccumulator> = HashMap::new();

        for layer in self
            .layers
            .iter()
            .filter(|layer| layer.weight > LAYER_REMOVE_EPSILON)
        {
            let clip = &self.clips[layer.clip_index];
            let binds = &self.bindings[layer.clip_index];
            let weight = layer.weight;
            let time = layer.time;

            for b in binds {
                let ch = &clip.channels[b.ch_index];
                let t = sample_translation(&ch.keys, time);
                let r = sample_rotation(&ch.keys, time);
                let s = sample_scale(&ch.keys, time);

                match b.target {
                    Binding::Transform(go) => {
                        if !go.exists() {
                            warn!("Animation game object was not found");
                            continue;
                        }

                        let pose = transform_locals.entry(go).or_default();
                        if let Some(t) = t {
                            pose.translation.add(t, weight);
                        }
                        if let Some(r) = r {
                            pose.rotation.add(r, weight);
                        }
                        if let Some(s) = s {
                            pose.scale.add(s, weight);
                        }
                    }
                    Binding::Bone { skel, idx } => {
                        if let Some(locals) = Self::ensure_pose_accumulator(skel, &mut skel_locals)
                        {
                            if idx >= locals.len() {
                                warn!("Binding bone index {idx} is out of range");
                                continue;
                            }

                            let pose = &mut locals[idx];
                            if let Some(t) = t {
                                pose.translation.add(t, weight);
                            }
                            if let Some(r) = r {
                                pose.rotation.add(r, weight);
                            }
                            if let Some(s) = s {
                                pose.scale.add(s, weight);
                            }
                        } else {
                            warn!("Binding bone not found");
                        }
                    }
                }
            }
        }

        for (mut go, pose) in transform_locals {
            if !go.exists() {
                warn!("Animation game object was not found");
                continue;
            }

            let tr = &mut go.transform;
            if let Some(t) = pose.translation.mixed() {
                tr.set_local_position_vec(
                    tr.local_position().lerp(t, pose.translation.blend_weight()),
                );
            }
            if let Some(r) = pose.rotation.mixed() {
                tr.set_local_rotation(tr.local_rotation().slerp(r, pose.rotation.blend_weight()));
            }
            if let Some(s) = pose.scale.mixed() {
                tr.set_nonuniform_local_scale(tr.local_scale().lerp(s, pose.scale.blend_weight()));
            }
        }

        for (skel_go, accum) in skel_locals {
            let Some(mut skel) = skel_go.get_component::<SkeletalComponent>() else {
                warn!("Skeleton not found on supposed Bone Channel Binding");
                continue;
            };

            let bones = skel.bones();
            let mut locals: Vec<SkeletonLocals> = Vec::with_capacity(bones.len());
            for (i, bind_local) in bones.bind_local.iter().enumerate() {
                let (bind_scale, bind_rotation, bind_translation) =
                    bind_local.to_scale_rotation_translation();
                let pose = accum.get(i).copied().unwrap_or_default();

                let translation = pose.translation.mixed().map_or(bind_translation, |t| {
                    bind_translation.lerp(t, pose.translation.blend_weight())
                });
                let rotation = pose.rotation.mixed().map_or(bind_rotation, |r| {
                    bind_rotation.slerp(r, pose.rotation.blend_weight())
                });
                let scale = pose.scale.mixed().map_or(bind_scale, |s| {
                    bind_scale.lerp(s, pose.scale.blend_weight())
                });

                locals.push((translation, rotation, scale));
            }

            skel.set_local_pose_trs(&locals);
        }
    }

    pub fn clips(&self) -> &[AnimationClip] {
        &self.clips
    }
}

fn collect_subtree_by_name(root: GameObjectId, out: &mut HashMap<String, GameObjectId>) {
    out.insert(root.name.clone(), root);
    for child in root.children().iter().copied() {
        collect_subtree_by_name(child, out);
    }
}
