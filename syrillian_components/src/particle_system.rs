use crate::proxy::particle_system::ParticleSystemProxy;
use std::time::Instant;
use syrillian::components::Component;
use syrillian::math::Vec3;
use syrillian::{Reflect, World};
use syrillian_render::proxies::SceneProxy;

#[repr(C)]
#[derive(Debug, Copy, Clone, Reflect)]
pub struct ParticleSystemSettings {
    /// Initial particle position
    pub position: Vec3,

    /// Initial particle velocity (when lifetime ends)
    pub velocity: Vec3,

    /// Additional acceleration per second
    pub acceleration: Vec3,

    /// Initial particle color
    pub color: Vec3,

    /// Final particle color
    pub end_color: Vec3,

    /// Particle lifetime in seconds
    pub lifetime: f32,

    /// Initial particle opacity
    pub opacity: f32,

    /// Final particle opacity
    pub end_opacity: f32,

    /// Randomness seed
    pub seed: u32,

    /// Emission rate in particles per second (after burst)
    pub spawn_rate: f32,

    /// Immediate particles emitted at t=0
    pub start_count: u32,

    /// Emitter duration in seconds
    pub duration: f32,

    /// Whether emission loops every duration
    pub looping: bool,

    /// Strength of turbulence displacement
    pub turbulence_strength: f32,

    /// Spatial frequency of turbulence
    pub turbulence_scale: f32,

    /// Speed of turbulence animation
    pub turbulence_speed: f32,

    /// Per-particle spawn position random range minimum
    pub position_random_min: Vec3,

    /// Per-particle spawn position random range maximum
    pub position_random_max: Vec3,

    /// Per-particle velocity random range minimum
    pub velocity_random_min: Vec3,

    /// Per-particle velocity random range maximum
    pub velocity_random_max: Vec3,

    /// Lifetime random multiplier minimum
    pub lifetime_random_min: f32,

    /// Lifetime random multiplier maximum
    pub lifetime_random_max: f32,
}

// TODO: derive Reflect when enums get added to reflections
#[derive(Debug)]
pub enum ParticleShape {
    Points,
}

#[derive(Debug, Reflect)]
#[reflect_all]
pub struct ParticleSystemComponent {
    pub shape: ParticleShape,
    pub data: ParticleSystemSettings,
    pub particle_count: u32,
}

impl Default for ParticleSystemComponent {
    fn default() -> Self {
        Self {
            shape: ParticleShape::Points,
            data: ParticleSystemSettings {
                position: Vec3::ZERO,
                velocity: Vec3::new(0.0, 1.0, 0.0),
                acceleration: Vec3::ZERO,
                color: Vec3::ONE,
                end_color: Vec3::ONE,
                lifetime: 10.0,
                opacity: 1.0,
                end_opacity: 1.0,
                seed: 0,
                spawn_rate: 1.0,
                start_count: 0,
                duration: 10.0,
                looping: true,
                turbulence_strength: 0.0,
                turbulence_scale: 0.1,
                turbulence_speed: 1.2,
                position_random_min: Vec3::ZERO,
                position_random_max: Vec3::ZERO,
                velocity_random_min: Vec3::ZERO,
                velocity_random_max: Vec3::ZERO,
                lifetime_random_min: 1.0,
                lifetime_random_max: 1.0,
            },
            particle_count: 10_000_000,
        }
    }
}

impl Component for ParticleSystemComponent {
    fn create_render_proxy(&mut self, _world: &World) -> Option<Box<dyn SceneProxy>> {
        Some(Box::new(ParticleSystemProxy {
            settings: self.data,
            particle_count: self.particle_count,
            start_time: Instant::now(),
        }))
    }
}
