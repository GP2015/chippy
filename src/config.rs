use alloc::{string::String, vec::Vec};
use log::error;
use serde::Deserialize;
use serde_with::serde_as;
use std::fs;
use winit::keyboard::{Key, SmolStr};

const CONFIG_FILE_PATH: &str = "config.toml";

#[serde_as]
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Config {
    pub cpu: CPUConfig,
    pub gpu: GPUConfig,
    pub input: InputConfig,
    pub ram: RAMConfig,
    pub sound_timer: SoundTimerConfig,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CPUConfig {
    pub instructions_per_second: f64,
    pub reset_flag_for_bitwise_operations: bool,
    pub use_new_shift_instruction: bool,
    pub use_new_jump_instruction: bool,
    pub set_flag_for_index_overflow: bool,
    pub move_index_with_reads: bool,
    pub limit_to_one_draw_per_frame: bool,
    pub allow_program_counter_overflow: bool,
    pub use_true_randomness: bool,
    pub fake_randomness_seed: u64,
    pub allow_index_register_overflow: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct GPUConfig {
    pub pixel_color_when_active: u32,
    pub pixel_color_when_inactive: u32,
    pub screen_border_color: u32,
    pub horizontal_resolution: u32,
    pub vertical_resolution: u32,
    pub wrap_sprite_positions: bool,
    pub wrap_sprite_pixels: bool,
}

fn deserialize_keys<'de, D>(deserializer: D) -> Result<[Key<SmolStr>; 16], D::Error>
where
    D: serde::Deserializer<'de>,
{
    Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .map(|key| Key::Character(SmolStr::new(key)))
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| serde::de::Error::custom("expected exactly 16 keys"))
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct InputConfig {
    #[serde(deserialize_with = "deserialize_keys")]
    pub key_bindings: [Key<SmolStr>; 16],
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct RAMConfig {
    pub allow_stack_overflow: bool,
    pub allow_heap_overflow: bool,
    pub font_starting_address: u16,
    #[serde_as(as = "[_; 80]")]
    pub font_data: [u8; 80],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToneWaveform {
    Sine,
    Square,
    Triangle,
    Sawtooth,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SoundTimerConfig {
    pub tone_frequency: f32,
    pub tone_waveform: ToneWaveform,
}

pub fn generate_configs() -> Option<Config> {
    let Ok(raw_config) = fs::read_to_string(CONFIG_FILE_PATH) else {
        error!("Could not read config.toml at {CONFIG_FILE_PATH}");
        return None;
    };

    toml::from_str(&raw_config)
        .map_err(|err| {
            error!("Could not parse config.toml ({err}).");
        })
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_configs() {
        let _ = generate_configs().unwrap();
    }
}
