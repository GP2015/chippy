use crate::{
    common::Limiter,
    config::{SoundTimerConfig, ToneWaveform},
};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use log::error;
use rodio::{
    DeviceSinkBuilder, MixerDeviceSink, Player,
    source::{SawtoothWave, SineWave, SquareWave, TriangleWave},
};

#[derive(Debug)]
pub struct DelayTimer {
    active: Arc<AtomicBool>,
    value: AtomicU8,
}

impl DelayTimer {
    pub fn new(active: Arc<AtomicBool>) -> Arc<Self> {
        Arc::new(Self {
            active,
            value: AtomicU8::new(0),
        })
    }

    pub fn run(&self) {
        let mut limiter = Limiter::new(60.0, true);

        while self.active.load(Ordering::Relaxed) {
            limiter.wait_if_early();

            let _ = self
                .value
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                    if v > 0 { Some(v - 1) } else { None }
                });
        }
    }

    pub fn get_value(&self) -> u8 {
        self.value.load(Ordering::Relaxed)
    }

    pub fn set_value(&self, val: u8) {
        self.value.store(val, Ordering::Relaxed);
    }
}

pub struct SoundTimer {
    active: Arc<AtomicBool>,
    value: AtomicU8,
    _handle: MixerDeviceSink,
    player: Player,
}

impl SoundTimer {
    pub fn try_new(active: Arc<AtomicBool>, config: &SoundTimerConfig) -> Option<Arc<Self>> {
        let mut handle = match DeviceSinkBuilder::open_default_sink() {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to open audio stream ({e}).");
                return None;
            }
        };

        handle.log_on_drop(false);

        let player = Player::connect_new(handle.mixer());
        player.pause();

        match config.tone_waveform {
            ToneWaveform::Sine => player.append(SineWave::new(config.tone_frequency)),
            ToneWaveform::Square => player.append(SquareWave::new(config.tone_frequency)),
            ToneWaveform::Triangle => player.append(TriangleWave::new(config.tone_frequency)),
            ToneWaveform::Sawtooth => player.append(SawtoothWave::new(config.tone_frequency)),
        }

        Some(Arc::new(Self {
            active,
            value: AtomicU8::new(0),
            _handle: handle,
            player,
        }))
    }

    #[cfg(test)]
    pub fn new_default(active: Arc<AtomicBool>) -> Arc<Self> {
        Self::try_new(
            active,
            &SoundTimerConfig {
                tone_frequency: 440.0,
                tone_waveform: ToneWaveform::Sine,
            },
        )
        .unwrap()
    }

    pub fn run(&self) {
        let mut limiter = Limiter::new(60.0, true);

        while self.active.load(Ordering::Relaxed) {
            limiter.wait_if_early();

            let _ = self
                .value
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                    if v > 0 { Some(v - 1) } else { None }
                });

            if self.value.load(Ordering::Relaxed) > 0 {
                self.player.play();
            } else {
                self.player.pause();
            }
        }
    }

    pub fn set_value(&self, val: u8) {
        self.value.store(val, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;
    use std::thread::{self, JoinHandle};

    fn create_delay_objects() -> (Arc<DelayTimer>, JoinHandle<()>, Arc<AtomicBool>) {
        let active = Arc::new(AtomicBool::new(true));
        let timer = DelayTimer::new(active.clone());
        let timer_clone = timer.clone();
        let handle = thread::spawn(move || timer_clone.run());
        (timer, handle, active)
    }

    #[test]
    fn test_delay_timer_decrement() {
        let (timer, handle, active) = create_delay_objects();

        timer.set_value(5);

        thread::sleep(Duration::from_millis(150));

        assert_eq!(0, timer.get_value());
        assert!(active.load(Ordering::Relaxed));

        active.store(false, Ordering::Relaxed);
        handle.join().unwrap();
    }
}
