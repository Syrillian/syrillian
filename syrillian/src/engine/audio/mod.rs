use kira::listener::ListenerHandle;
use kira::track::{SpatialTrackBuilder, SpatialTrackHandle};
use kira::{AudioManager, AudioManagerSettings, DefaultBackend, Tween};
use tracing::error;

use crate::math::Vec3;
pub use kira::effect;
pub use kira::track;
use syrillian::math::Quat;

struct AudioSceneInner {
    manager: AudioManager<DefaultBackend>,
    listener: ListenerHandle,
}

impl AudioSceneInner {
    fn new() -> Option<Self> {
        let mut manager = match AudioManager::new(AudioManagerSettings::default()) {
            Ok(x) => x,
            Err(e) => {
                error!("Audio manager could not be initialized: {e:?}");
                return None;
            }
        };

        let position = Vec3::ZERO;
        let orientation = Quat::IDENTITY;

        let listener = match manager.add_listener(position, orientation) {
            Ok(x) => x,
            Err(e) => {
                // So we technically have an audio manager but can't play anything. Fantastic.
                error!("Failed to add audio listener: {e}");
                return None;
            }
        };

        Some(Self { manager, listener })
    }
}

pub struct AudioScene {
    inner: Option<AudioSceneInner>,
}

impl Default for AudioScene {
    fn default() -> Self {
        Self {
            inner: AudioSceneInner::new(),
        }
    }
}

impl AudioScene {
    pub fn set_receiver_position(&mut self, receiver_position: Vec3) {
        if let Some(this) = self.inner.as_mut() {
            this.listener
                .set_position(receiver_position, Tween::default())
        }
    }

    pub fn set_receiver_orientation(&mut self, receiver_orientation: Quat) {
        if let Some(this) = self.inner.as_mut() {
            this.listener
                .set_orientation(receiver_orientation, Tween::default())
        }
    }

    /// Returns none if the spatial track limit was reached
    pub fn add_spatial_track(
        &mut self,
        initial_position: Vec3,
        track: SpatialTrackBuilder,
    ) -> Option<SpatialTrackHandle> {
        self.inner.as_mut().and_then(|this| {
            this.manager
                .add_spatial_sub_track(this.listener.id(), initial_position, track)
                .ok()
        })
    }
}
