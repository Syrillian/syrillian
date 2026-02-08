use crate::store::{H, HandleName, StoreType};
use delegate::delegate;
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use kira::sound::{IntoOptionalRegion, PlaybackPosition};
use kira::{Decibels, Frame, Panning, PlaybackRate, StartTime, Tween, Value};
use std::error::Error;
use std::io::Cursor;
use web_time::Duration;

#[derive(Debug, Clone)]
pub struct Sound {
    inner: StaticSoundData,
}

impl StoreType for Sound {
    fn name() -> &'static str {
        "Sound"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}

impl Sound {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_sound(path: &str) -> Result<Sound, Box<dyn Error>> {
        let data = StaticSoundData::from_file(path)?;

        let sound = Sound { inner: data };

        Ok(sound)
    }

    pub fn load_sound_data(sound: Vec<u8>) -> Result<Sound, Box<dyn Error>> {
        let data = StaticSoundData::from_cursor(Cursor::new(sound))?;

        let sound = Sound { inner: data };

        Ok(sound)
    }

    delegate! {
        to self.inner {
            #[expr(self.inner.sample_rate)]
            pub fn sample_rate(&self) -> u32;

            #[call(num_frames)]
            pub fn frames(&self) -> usize;

            pub fn duration(&self) -> Duration;
            pub fn unsliced_duration(&self) -> Duration;
            pub fn frame_at_index(&self, index: usize) -> Option<Frame>;

            #[expr(self.inner = $; self)]
            #[call(start_time)]
            pub fn set_start_time(&mut self, start_time: impl Into<StartTime>) -> &mut Self;

            #[expr(self.inner = $; self)]
            #[call(start_position)]
            pub fn set_start_position(&mut self, start_position: impl Into<PlaybackPosition>) -> &mut Self;

            #[expr(self.inner = $; self)]
            pub fn reverse(&mut self, reverse: bool) -> &mut Self;

            #[expr(self.inner = $; self)]
            #[call(volume)]
            pub fn set_volume(&mut self, volume: impl Into<Value<Decibels>>) -> &mut Self;

            #[expr(self.inner = $; self)]
            #[call(playback_rate)]
            pub fn set_speed(&mut self, speed: impl Into<Value<PlaybackRate>>) -> &mut Self;

            #[expr(self.inner = $; self)]
            #[call(panning)]
            pub fn set_panning(&mut self, panning: impl Into<Value<Panning>>) -> &mut Self;

            #[expr(self.inner = $; self)]
            #[call(fade_in_tween)]
            pub fn set_fade_in(&mut self, tween: impl Into<Option<Tween>>) -> &mut Self;

            #[expr(self.inner = $; self)]
            pub fn slice(&mut self, slice: impl IntoOptionalRegion) -> &mut Self;
        }
    }

    pub fn with_settings(&self, settings: StaticSoundSettings) -> Self {
        Sound {
            inner: self.inner.with_settings(settings),
        }
    }

    pub fn inner(&self) -> StaticSoundData {
        self.inner.clone()
    }

    pub fn from_data(data: StaticSoundData) -> Self {
        Sound { inner: data }
    }
}
