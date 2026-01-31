use crate::World;
use std::collections::VecDeque;

const DEFAULT_RUNNING_SIZE: usize = 60;

#[derive(Debug, Clone, Default)]
pub struct FrameCounter {
    frame_times: VecDeque<f32>,
}

impl FrameCounter {
    pub fn new_frame(&mut self, delta_time: f32) {
        if self.frame_times.len() >= DEFAULT_RUNNING_SIZE {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(delta_time);
    }

    pub fn new_frame_from_world(&mut self, world: &World) {
        let frame_time = world.delta_time().as_secs_f32();
        self.new_frame(frame_time);
    }

    pub fn delta_mean(&self) -> f32 {
        self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32
    }

    pub fn fps_mean(&self) -> u32 {
        (1.0 / self.delta_mean()) as u32
    }

    pub fn fps_low(&self) -> u32 {
        (1.0 / self.delta_high()) as u32
    }

    pub fn fps_high(&self) -> u32 {
        (1.0 / self.delta_low()) as u32
    }

    pub fn delta_low(&self) -> f32 {
        self.frame_times
            .iter()
            .copied()
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or_default()
    }

    pub fn delta_high(&self) -> f32 {
        self.frame_times
            .iter()
            .copied()
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_mean_over_sliding_window() {
        let mut counter = FrameCounter::default();
        for _ in 0..DEFAULT_RUNNING_SIZE {
            counter.new_frame(0.01);
        }

        // push values that force eviction of the oldest entries
        for _ in 0..5 {
            counter.new_frame(0.02);
        }

        assert_eq!(counter.frame_times.len(), DEFAULT_RUNNING_SIZE);
        // last 5 are 0.02, preceding 55 are 0.01 -> mean should reflect both
        let expected = (55.0 * 0.01 + 5.0 * 0.02) / DEFAULT_RUNNING_SIZE as f32;
        assert!((counter.delta_mean() - expected).abs() < 1e-6);
        assert_eq!(counter.fps_mean(), (1.0 / expected) as u32);
    }
}
