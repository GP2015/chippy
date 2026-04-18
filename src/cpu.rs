use crate::config::CPUConfig;
use crate::emulib::Limiter;
use crate::gpu::Gpu;
use crate::input::InputManager;
use crate::instructions::{self, InstructionFunction, Opcode};
use crate::ram::{PROGRAM_START_ADDRESS, Ram};
use crate::timer::{DelayTimer, SoundTimer};
use std::ops::{Bound, RangeBounds};
use std::slice::SliceIndex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

pub struct Cpu {
    pub active: Arc<AtomicBool>,
    pub config: CPUConfig,
    pub gpu: Arc<Gpu>,
    pub ram: Arc<Ram>,
    pub delay_timer: Arc<DelayTimer>,
    pub sound_timer: Arc<SoundTimer>,
    pub input_manager: Arc<InputManager>,
    pc: Mutex<u16>,
    index: Mutex<u16>,
    v: Mutex<[u8; 16]>,
}

impl Cpu {
    pub fn try_new(
        active: Arc<AtomicBool>,
        config: CPUConfig,
        gpu: Arc<Gpu>,
        ram: Arc<Ram>,
        delay_timer: Arc<DelayTimer>,
        sound_timer: Arc<SoundTimer>,
        input_manager: Arc<InputManager>,
    ) -> Option<Arc<Self>> {
        if config.instructions_per_second <= 0.0 {
            eprintln!("The CPU's instruction-per-second rate must be greater than 0.");
            active.store(false, Ordering::Relaxed);
            return None;
        }

        Some(Arc::new(Self {
            active,
            config,
            gpu,
            ram,
            delay_timer,
            sound_timer,
            input_manager,
            pc: Mutex::new(PROGRAM_START_ADDRESS),
            index: Mutex::new(0),
            v: Mutex::new([0; 16]),
        }))
    }

    #[cfg(test)]
    pub fn new_default_all_false(
        active: Arc<AtomicBool>,
        gpu: Arc<Gpu>,
        ram: Arc<Ram>,
        delay_timer: Arc<DelayTimer>,
        sound_timer: Arc<SoundTimer>,
        input_manager: Arc<InputManager>,
    ) -> Arc<Self> {
        Self::try_new(
            active,
            CPUConfig {
                instructions_per_second: 700.0,
                reset_flag_for_bitwise_operations: false,
                use_new_shift_instruction: false,
                use_new_jump_instruction: false,
                set_flag_for_index_overflow: false,
                move_index_with_reads: false,
                limit_to_one_draw_per_frame: false,
                allow_program_counter_overflow: false,
                use_true_randomness: false,
                fake_randomness_seed: 0,
                allow_index_register_overflow: false,
            },
            gpu,
            ram,
            delay_timer,
            sound_timer,
            input_manager,
        )
        .unwrap()
    }

    #[cfg(test)]
    pub fn new_default_all_true(
        active: Arc<AtomicBool>,
        gpu: Arc<Gpu>,
        ram: Arc<Ram>,
        delay_timer: Arc<DelayTimer>,
        sound_timer: Arc<SoundTimer>,
        input_manager: Arc<InputManager>,
    ) -> Arc<Self> {
        Self::try_new(
            active,
            CPUConfig {
                instructions_per_second: 700.0,
                reset_flag_for_bitwise_operations: true,
                use_new_shift_instruction: true,
                use_new_jump_instruction: true,
                set_flag_for_index_overflow: true,
                move_index_with_reads: true,
                limit_to_one_draw_per_frame: true,
                allow_program_counter_overflow: true,
                use_true_randomness: true,
                fake_randomness_seed: 0,
                allow_index_register_overflow: true,
            },
            gpu,
            ram,
            delay_timer,
            sound_timer,
            input_manager,
        )
        .unwrap()
    }

    pub fn run(&self) {
        if !self.config.use_true_randomness {
            fastrand::seed(self.config.fake_randomness_seed);
        }

        let mut limiter = Limiter::new(self.config.instructions_per_second, true);

        while self.active.load(Ordering::Relaxed) {
            limiter.wait_if_early();

            let Some(instruction) = self.fetch_instruction() else {
                return;
            };

            let Some(function) = Self::decode_instruction(&instruction) else {
                continue;
            };

            if self.execute_instruction(&instruction, function) {
                limiter.reset();
            }
        }
    }

    fn fetch_instruction(&self) -> Option<Opcode> {
        let mut pc = self.pc.lock().unwrap();

        if *pc >= 0xFFE && !self.config.allow_program_counter_overflow {
            eprintln!("Program counter overflowed.");
            self.active.store(false, Ordering::Relaxed);
            return None;
        }

        let instruction_bytes = self.ram.read_bytes(*pc, 2)?;

        *pc = (*pc + 2) % 0x1000;

        Some(Opcode::from_u8s(instruction_bytes[0], instruction_bytes[1]))
    }

    fn decode_instruction(instruction: &Opcode) -> Option<InstructionFunction> {
        instructions::get_instruction_function(instruction)
    }

    fn execute_instruction(&self, instruction: &Opcode, function: InstructionFunction) -> bool {
        function(self, instruction)
    }

    pub fn get_pc_ref(&self) -> MutexGuard<'_, u16> {
        self.pc.lock().unwrap()
    }

    // pub fn get_pc(&self) -> u16 {
    //     return *self.pc.lock().unwrap();
    // }

    pub fn set_pc(&self, value: u16) {
        debug_assert!(
            value <= 0xFFF,
            "Should not be possible to manually set program counter outside address space."
        );

        *self.pc.lock().unwrap() = value;
    }

    pub fn increment_pc(&self) -> bool {
        let mut pc = self.pc.lock().unwrap();

        if *pc >= 0xFFE && !self.config.allow_program_counter_overflow {
            eprintln!("Program counter overflowed.");
            self.active.store(false, Ordering::Relaxed);
            return false;
        }

        *pc = (*pc + 2) % 0x1000;
        true
    }

    pub fn get_index_reg_ref(&self) -> MutexGuard<'_, u16> {
        self.index.lock().unwrap()
    }

    pub fn get_index_reg(&self) -> u16 {
        return *self.index.lock().unwrap();
    }

    pub fn set_index_reg(&self, value: u16) {
        debug_assert!(
            value <= 0xFFF,
            "Should not be possible to manually set index register outside address space."
        );

        *self.index.lock().unwrap() = value;
    }

    pub fn increment_index_reg_by(&self, value: u16) -> Option<bool> {
        let index = self.index.lock().unwrap();
        self.increment_index_reg_ref_by(index, value)
    }

    pub fn increment_index_reg_ref_by(
        &self,
        mut index_ref: MutexGuard<'_, u16>,
        value: u16,
    ) -> Option<bool> {
        let (val, wrapped) = index_ref.overflowing_add(value);

        if wrapped && !self.config.allow_index_register_overflow {
            eprintln!("Index register overflowed.");
            self.active.store(false, Ordering::Relaxed);
            return None;
        }

        *index_ref = val;

        Some(*index_ref > 0xFFF)
    }

    pub fn get_v_regs_ref(&self) -> MutexGuard<'_, [u8; 16]> {
        self.v.lock().unwrap()
    }

    pub fn get_v_reg(&self, reg: u8) -> u8 {
        debug_assert!(
            reg <= 0xF,
            "Should not be possible to access non-existent V registers."
        );

        self.v.lock().unwrap()[reg as usize]
    }

    pub fn get_v_reg_xy(&self, x: u8, y: u8) -> (u8, u8) {
        debug_assert!(
            x <= 0xF && y <= 0xF,
            "Should not be possible to access non-existent V registers."
        );

        let v = self.v.lock().unwrap();
        (v[x as usize], v[y as usize])
    }

    pub fn get_v_reg_range<R>(&self, range: R) -> Vec<u8>
    where
        R: SliceIndex<[u8], Output = [u8]> + RangeBounds<usize>,
    {
        let v = self.v.lock().unwrap();

        if cfg!(debug_assertions) {
            let start = match range.start_bound() {
                Bound::Included(&s) => s,
                Bound::Excluded(&s) => s + 1,
                Bound::Unbounded => 0,
            };

            let end = match range.end_bound() {
                Bound::Included(&e) => e,
                Bound::Excluded(&e) => e.saturating_sub(1),
                Bound::Unbounded => v.len() - 1,
            };

            debug_assert!(
                start <= 0xF && end <= 0xF,
                "Should not be possible to access non-existent V registers."
            );
        }

        v[range].to_vec()
    }

    pub fn set_v_reg(&self, reg: u8, val: u8) {
        debug_assert!(
            reg <= 0xF,
            "Should not be possible to access non-existent V registers."
        );

        self.v.lock().unwrap()[reg as usize] = val;
    }

    pub fn set_v_reg_range(&self, reg: u8, vals: &[u8]) {
        let reg = reg as usize;

        debug_assert!(
            reg + vals.len() - 1 <= 0xF,
            "Should not be possible to access non-existent V registers."
        );

        self.v.lock().unwrap()[reg..reg + vals.len()].copy_from_slice(vals);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum ConfigType {
        Conservative,
        Liberal,
    }

    fn create_objects(cfg_type: ConfigType) -> (Arc<Cpu>, Arc<AtomicBool>) {
        let active = Arc::new(AtomicBool::new(true));

        let delay_timer = DelayTimer::new_default(active.clone());
        let sound_timer = SoundTimer::new_default(active.clone());
        let ram = Ram::new_default_conservative(active.clone());
        let gpu = Gpu::new_default_wrapping(active.clone());
        let input_manager = InputManager::new_default(active.clone());
        let cpu = match cfg_type {
            ConfigType::Conservative => Cpu::new_default_all_false(
                active.clone(),
                gpu,
                ram,
                delay_timer,
                sound_timer,
                input_manager,
            ),
            ConfigType::Liberal => Cpu::new_default_all_true(
                active.clone(),
                gpu,
                ram,
                delay_timer,
                sound_timer,
                input_manager,
            ),
        };

        (cpu, active)
    }

    #[test]
    fn test_increment_program_counter() {
        let (cpu, active) = create_objects(ConfigType::Conservative);

        let old_val = *cpu.pc.lock().unwrap();

        for _ in 0..5 {
            assert!(cpu.increment_pc());
        }

        assert_eq!(old_val + 10, *cpu.pc.lock().unwrap());
        assert!(active.load(Ordering::Relaxed));
    }

    #[test]
    fn test_successful_program_counter_overflow() {
        let (cpu, active) = create_objects(ConfigType::Liberal);

        for _ in 0..((0x1000 - PROGRAM_START_ADDRESS) / 2) {
            assert!(cpu.increment_pc());
        }

        assert_eq!(0x000, *cpu.pc.lock().unwrap());
        assert!(active.load(Ordering::Relaxed));
    }

    #[test]
    fn test_failed_program_counter_overflow() {
        let (cpu, active) = create_objects(ConfigType::Conservative);

        for _ in 0..((0xFFF - PROGRAM_START_ADDRESS) / 2) {
            assert!(cpu.increment_pc());
        }

        assert!(!cpu.increment_pc());
        assert!(!active.load(Ordering::Relaxed));
    }

    #[test]
    fn test_set_program_counter_manually() {
        let (cpu, active) = create_objects(ConfigType::Conservative);
        cpu.set_pc(0x567);
        assert_eq!(0x567, *cpu.get_pc_ref());
        assert!(active.load(Ordering::Relaxed));
    }

    #[test]
    fn test_get_program_counter_reference() {
        let (cpu, active) = create_objects(ConfigType::Conservative);

        {
            let mut pc = cpu.get_pc_ref();
            *pc = 0x567;
        }

        let mut pc = cpu.get_pc_ref();
        *pc += 2;

        assert_eq!(0x569, *pc);
        assert!(active.load(Ordering::Relaxed));
    }

    #[test]
    fn test_set_index_register() {
        let (cpu, active) = create_objects(ConfigType::Conservative);
        cpu.set_index_reg(0x567);
        assert_eq!(0x567, cpu.get_index_reg());
        assert!(active.load(Ordering::Relaxed));
    }

    #[test]
    fn test_set_v_register() {
        let (cpu, active) = create_objects(ConfigType::Conservative);
        cpu.set_v_reg(0x5, 0x67);
        assert_eq!(0x67, cpu.get_v_reg(0x5));
        assert!(active.load(Ordering::Relaxed));
    }

    #[test]
    fn test_get_v_register_reference() {
        let (cpu, active) = create_objects(ConfigType::Conservative);

        {
            let mut v = cpu.get_v_regs_ref();
            v[0x5] = 0x67;
        }

        let v = cpu.get_v_regs_ref();
        assert_eq!(0x67, v[0x5]);
        assert!(active.load(Ordering::Relaxed));
    }

    #[test]
    fn test_get_v_register_for_x_and_y() {
        let (cpu, active) = create_objects(ConfigType::Conservative);
        cpu.set_v_reg(2, 0x34);
        cpu.set_v_reg(5, 0x67);
        assert_eq!((0x34, 0x67), cpu.get_v_reg_xy(2, 5));
        assert!(active.load(Ordering::Relaxed));
    }

    #[test]
    fn test_set_v_register_range() {
        let (cpu, active) = create_objects(ConfigType::Conservative);

        let ideal_bytes = vec![0x48, 0x65, 0x6c, 0x6c, 0x6f];
        cpu.set_v_reg_range(2, &ideal_bytes);

        for i in 0..5 {
            assert_eq!(ideal_bytes[i as usize], cpu.get_v_reg(i + 2));
        }

        assert!(active.load(Ordering::Relaxed));
    }

    #[test]
    fn test_get_v_register_range() {
        let (cpu, active) = create_objects(ConfigType::Conservative);

        let ideal_bytes = vec![0x48, 0x65, 0x6c, 0x6c, 0x6f];

        for i in 0..5 {
            cpu.set_v_reg(i + 2, ideal_bytes[i as usize]);
        }

        assert_eq!(ideal_bytes, cpu.get_v_reg_range(2..7));
        assert!(active.load(Ordering::Relaxed));
    }
}
