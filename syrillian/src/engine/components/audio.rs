use crate::Reflect;
use crate::World;
use crate::assets::HSound;
use crate::components::Component;
use kira::Tween;
use kira::sound::PlaybackState;
use kira::sound::static_sound::StaticSoundHandle;
use kira::track::{SpatialTrackBuilder, SpatialTrackHandle};
use tracing::{trace, warn};

#[derive(Debug, Default, Reflect)]
pub struct AudioReceiver;

#[derive(Debug, Default, Reflect)]
pub struct AudioEmitter {
    asset_handle: Option<HSound>,
    sound_handle: Option<StaticSoundHandle>,
    track_handle: Option<SpatialTrackHandle>,
    looping: bool,
    play_triggered: bool,
}

impl Component for AudioEmitter {
    fn init(&mut self, world: &mut World) {
        trace!("Initializing new Spatial Track");
        self.track_handle = world.audio.add_spatial_track(
            self.parent().transform.position(),
            SpatialTrackBuilder::new(),
        );
    }

    fn update(&mut self, world: &mut World) {
        let position = self.parent().transform.position();

        let Some(track) = self.track_handle.as_mut() else {
            return;
        };

        track.set_position(position, Tween::default());

        if self.play_triggered || (self.looping && !self.is_playing()) {
            self._play(world);
        }
    }
}

impl AudioEmitter {
    pub fn toggle_play(&mut self) {
        if self.is_playing() {
            self.play();
        } else {
            self.stop();
        }
    }

    pub fn play(&mut self) {
        self.play_triggered = true;
    }

    fn _play(&mut self, world: &mut World) {
        if self.is_playing() {
            self.stop();
            debug_assert!(self.sound_handle.is_none());
        }

        let Some(track) = self.track_handle.as_mut() else {
            return;
        };

        let Some(h) = self.asset_handle else {
            warn!("AudioEmitter play had no asset handle");
            return;
        };

        let Some(sound) = world.assets.sounds.try_get(h) else {
            warn!("AudioEmitter play had no sound handle");
            return;
        };

        self.play_triggered = false;

        match track.play(sound.inner()) {
            Ok(handle) => self.sound_handle = Some(handle),
            Err(e) => {
                warn!("Error when playing sound: {e}")
            }
        }
    }

    pub fn toggle_looping(&mut self) {
        self.set_looping(!self.looping)
    }

    pub fn set_looping(&mut self, looping: bool) {
        if self.looping && !looping {
            self.stop();
        }
        self.looping = looping;
    }

    pub fn stop(&mut self) {
        self.stop_fade(Tween::default())
    }

    pub fn stop_fade(&mut self, tween: Tween) {
        if let Some(mut handle) = self.sound_handle.take() {
            handle.stop(tween);
        }
    }

    pub fn is_playing(&self) -> bool {
        self.sound_handle
            .as_ref()
            .is_some_and(|p| p.state() == PlaybackState::Playing)
    }

    pub fn set_sound(&mut self, sound: HSound) {
        self.stop();
        debug_assert_eq!(self.asset_handle, None);
        self.asset_handle = Some(sound);
    }

    pub fn set_track(&mut self, world: &mut World, track: SpatialTrackBuilder) -> &mut Self {
        let pos = self.parent().transform.position();
        self.track_handle = world.audio.add_spatial_track(pos, track);
        if self.track_handle.is_none() {
            warn!("Spatial track limit reached");
        }
        self
    }
}

impl Component for AudioReceiver {
    fn update(&mut self, world: &mut World) {
        let transform = &self.parent().transform;

        world.audio.set_receiver_position(transform.position());
        world.audio.set_receiver_orientation(*transform.rotation());
    }
}
