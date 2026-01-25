use crate::World;
use crate::components::{Component, SkeletalComponent};
use crate::core::GameObjectId;
use crate::utils::ExtraMatrixMath;
use crate::utils::animation::{
    AnimationClip, Binding, ChannelBinding, ClipIndex, Playback, sample_rotation, sample_scale,
    sample_translation,
};
use nalgebra::{UnitQuaternion, Vector3};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use syrillian_macros::Reflect;
use tracing::warn;

#[derive(Default, Reflect)]
pub struct AnimationComponent {
    // Multiple clips (by name)
    clips: Vec<AnimationClip>,
    clip_indices: Vec<ClipIndex>,

    // Active playback stack
    current: Option<Playback>,

    bindings: Vec<Vec<ChannelBinding>>,
}

/// Position, Rotation, Scale
type SkeletonLocals = (Vector3<f32>, UnitQuaternion<f32>, Vector3<f32>);

impl Component for AnimationComponent {
    fn update(&mut self, world: &mut World) {
        let Some(pb) = self.current.as_mut() else {
            return;
        };
        if self.clips.is_empty() {
            return;
        }

        let dt = world.delta_time().as_secs_f32();
        pb.time += dt * pb.speed;

        let clip = &self.clips[pb.clip_index];
        if clip.duration > 0.0 {
            if pb.looping {
                pb.time = pb.time.rem_euclid(clip.duration);
            } else if pb.time > clip.duration {
                pb.time = clip.duration;
            }
        }

        let clip_index = pb.clip_index;
        let time = pb.time;
        let weight = pb.weight;

        self.evaluate_and_apply(clip_index, time, weight);
    }
}

impl AnimationComponent {
    pub fn set_clips(&mut self, clips: Vec<AnimationClip>) {
        let clip_indices = clips.iter().map(ClipIndex::new).collect();
        self.clips = clips;
        self.clip_indices = clip_indices;
        self.resolve_bindings();
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
        if let Some((idx, _)) = self.clips.iter().enumerate().find(|(_, c)| c.name == name) {
            self.current = Some(Playback {
                clip_index: idx,
                time: 0.0,
                speed,
                weight,
                looping,
            });
        }
    }

    pub fn play_index(&mut self, index: usize, looping: bool, speed: f32, weight: f32) {
        if index >= self.clips.len() {
            warn!("No clip #{index} found in {}", self.parent().name);
            return;
        }

        self.current = Some(Playback {
            clip_index: index,
            time: 0.0,
            speed,
            weight,
            looping,
        });
    }

    fn ensure_pose(
        skel_go: GameObjectId,
        locals: &mut HashMap<GameObjectId, Vec<SkeletonLocals>>,
    ) -> Option<&mut Vec<SkeletonLocals>> {
        if !skel_go.exists() {
            return None;
        }

        match locals.entry(skel_go) {
            Entry::Occupied(o) => Some(o.into_mut()),
            Entry::Vacant(e) => {
                let skel = skel_go.get_component::<SkeletalComponent>()?;
                let bones = skel.bones();
                let mut pose = Vec::with_capacity(bones.len());
                for m in &bones.bind_local {
                    let (t, r, s) = m.decompose();
                    pose.push((t, r, s));
                }
                Some(e.insert(pose))
            }
        }
    }

    fn evaluate_and_apply(&mut self, clip_index: usize, time: f32, weight: f32) {
        let clip = &self.clips[clip_index];
        let binds = &self.bindings[clip_index];

        let mut skel_locals: HashMap<GameObjectId, Vec<SkeletonLocals>> = HashMap::new();

        for b in binds {
            let ch = &clip.channels[b.ch_index];
            let t = sample_translation(&ch.keys, time);
            let r = sample_rotation(&ch.keys, time);
            let s = sample_scale(&ch.keys, time);

            match b.target {
                Binding::Transform(mut go) => {
                    if !go.exists() {
                        warn!("Animation game object was not found");
                        continue;
                    }

                    let tr = &mut go.transform;
                    if let Some(t) = t {
                        tr.set_local_position_vec(tr.local_position().lerp(&t, weight));
                    }
                    if let Some(r) = r {
                        tr.set_local_rotation(tr.local_rotation().slerp(&r, weight));
                    }
                    if let Some(s) = s {
                        let cur_s = tr.local_scale();
                        tr.set_nonuniform_local_scale(cur_s.lerp(&s, weight));
                    }
                }
                Binding::Bone { skel, idx } => {
                    if let Some(locals) = Self::ensure_pose(skel, &mut skel_locals) {
                        let (lt, lr, ls) = &mut locals[idx];
                        if let Some(t) = t {
                            *lt = lt.lerp(&t, weight);
                        }
                        if let Some(r) = r {
                            *lr = lr.slerp(&r, weight);
                        }
                        if let Some(s) = s {
                            *ls = ls.lerp(&s, weight);
                        }
                    } else {
                        warn!("Binding bone not found");
                    }
                }
            }
        }

        for (skel_go, locals) in skel_locals {
            if let Some(mut skel) = skel_go.get_component::<SkeletalComponent>() {
                skel.set_local_pose_trs(&locals);
            } else {
                warn!("Skeleton not found on supposed Bone Channel Binding");
            }
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
