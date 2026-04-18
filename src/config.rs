use serde::Deserialize;
use serde_with::serde_as;
use std::fs;
use winit::keyboard::{Key, SmolStr};

const CONFIG_FILE_PATH: &str = "config.toml";

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    CHIP8,
    Custom,
}

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct Config {
    pub preset: Preset,
    pub cpu: CPUConfig,
    pub gpu: GPUConfig,
    pub input: InputConfig,
    pub ram: RAMConfig,
    pub delay_timer: DelayTimerConfig,
    pub sound_timer: SoundTimerConfig,
}

#[derive(Deserialize, Debug)]
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

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenderOccasion {
    Changes,
    Frequency,
}

#[derive(Deserialize, Debug)]
pub struct GPUConfig {
    pub pixel_color_when_active: u32,
    pub pixel_color_when_inactive: u32,
    pub screen_border_color: u32,
    pub horizontal_resolution: usize,
    pub vertical_resolution: usize,
    pub wrap_sprite_positions: bool,
    pub wrap_sprite_pixels: bool,
    pub render_occasion: RenderOccasion,
    pub render_frequency: f64,
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

#[derive(Deserialize, Debug)]
pub struct InputConfig {
    #[serde(deserialize_with = "deserialize_keys")]
    pub key_bindings: [Key<SmolStr>; 16],
}

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct RAMConfig {
    pub stack_size: usize,
    pub allow_stack_overflow: bool,
    pub allow_heap_overflow: bool,
    pub font_starting_address: u16,
    #[serde_as(as = "[_; 80]")]
    pub font_data: [u8; 80],
}

#[derive(Deserialize, Debug)]
pub struct DelayTimerConfig {
    pub delay_timer_decrement_rate: f64,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToneWaveform {
    Sine,
    Square,
    Triangle,
    Sawtooth,
}

#[derive(Deserialize, Debug)]
pub struct SoundTimerConfig {
    pub sound_timer_decrement_rate: f64,
    pub tone_frequency: f32,
    pub tone_waveform: ToneWaveform,
}

pub fn generate_configs() -> Option<Config> {
    let Ok(raw_config) = fs::read_to_string(CONFIG_FILE_PATH) else {
        eprintln!("Could not read config.toml at {CONFIG_FILE_PATH}");
        return None;
    };

    let mut config: Config = toml::from_str(&raw_config)
        .map_err(|err| {
            eprintln!("Could not parse config.toml ({err}).");
        })
        .ok()?;

    match config.preset {
        Preset::CHIP8 => enable_chip8_preset(&mut config),
        Preset::Custom => (),
    }

    Some(config)
}

fn enable_chip8_preset(config: &mut Config) {
    config.cpu.reset_flag_for_bitwise_operations = true;
    config.cpu.use_new_shift_instruction = false;
    config.cpu.use_new_jump_instruction = false;
    config.cpu.set_flag_for_index_overflow = false;
    config.cpu.move_index_with_reads = true;
    config.cpu.limit_to_one_draw_per_frame = true;
    config.gpu.horizontal_resolution = 64;
    config.gpu.vertical_resolution = 32;
    config.gpu.wrap_sprite_positions = true;
    config.gpu.wrap_sprite_pixels = false;
    config.gpu.render_occasion = RenderOccasion::Frequency;
    config.gpu.render_frequency = 60.0;
    config.ram.stack_size = 16;
    config.delay_timer.delay_timer_decrement_rate = 60.0;
    config.sound_timer.sound_timer_decrement_rate = 60.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_configs() {
        let _ = generate_configs().unwrap();
    }
}
