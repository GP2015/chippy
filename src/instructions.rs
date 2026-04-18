use crate::cpu::Cpu;

pub struct Opcode {
    full: u16,
}

impl Opcode {
    // pub fn from_u16(full: u16) -> Self {
    //     Self { full }
    // }

    pub fn from_u8s(high: u8, low: u8) -> Self {
        Self {
            full: (u16::from(high) << 8) | u16::from(low),
        }
    }

    // pub fn get_full(&self) -> u16 {
    //     self.full
    // }

    pub fn get_addr(&self) -> u16 {
        self.full & 0x0FFF
    }

    pub fn get_kk(&self) -> u8 {
        (self.full & 0x00FF) as u8
    }

    pub fn get_s(&self) -> u8 {
        ((self.full & 0xF000) >> 12) as u8
    }

    pub fn get_x(&self) -> u8 {
        ((self.full & 0x0F00) >> 8) as u8
    }

    pub fn get_x_usize(&self) -> usize {
        self.get_x() as usize
    }

    pub fn get_y(&self) -> u8 {
        ((self.full & 0x00F0) >> 4) as u8
    }

    pub fn get_y_usize(&self) -> usize {
        self.get_y() as usize
    }

    // pub fn get_x_and_y(&self) -> (u8, u8) {
    //     (self.get_x(), self.get_y())
    // }

    pub fn get_x_and_y_usize(&self) -> (usize, usize) {
        (self.get_x_usize(), self.get_y_usize())
    }

    pub fn get_n(&self) -> u8 {
        (self.full & 0x000F) as u8
    }
}

pub type InstructionFunction = fn(&Cpu, &Opcode) -> bool;

pub fn get_instruction_function(op: &Opcode) -> Option<InstructionFunction> {
    match op.get_s() {
        0x0 => match op.get_addr() {
            0x0E0 => Some(i_00E0_CLS),
            0x0EE => Some(i_00EE_RET),
            _ => {
                eprintln!("Machine code routines are not supported.");
                None
            }
        },

        0x1 => Some(i_1nnn_JP_addr),
        0x2 => Some(i_2nnn_CALL_addr),
        0x3 => Some(i_3xkk_SE_Vx_byte),
        0x4 => Some(i_4xkk_SNE_Vx_byte),

        #[allow(clippy::single_match_else)]
        0x5 => match op.get_n() {
            0x0 => Some(i_5xy0_SE_Vx_Vy),
            _ => {
                invalid_instruction_called();
                None
            }
        },

        0x6 => Some(i_6xkk_LD_Vx_byte),
        0x7 => Some(i_7xkk_ADD_Vx_byte),

        0x8 => match op.get_n() {
            0x0 => Some(i_8xy0_LD_Vx_Vy),
            0x1 => Some(i_8xy1_OR_Vx_Vy),
            0x2 => Some(i_8xy2_AND_Vx_Vy),
            0x3 => Some(i_8xy3_XOR_Vx_Vy),
            0x4 => Some(i_8xy4_ADD_Vx_Vy),
            0x5 => Some(i_8xy5_SUB_Vx_Vy),
            0x6 => Some(i_8xy6_SHR_Vx),
            0x7 => Some(i_8xy7_SUBN_Vx_Vy),
            0xE => Some(i_8xyE_SHL_Vx),
            _ => {
                invalid_instruction_called();
                None
            }
        },

        #[allow(clippy::single_match_else)]
        0x9 => match op.get_n() {
            0x0 => Some(i_9xy0_SNE_Vx_Vy),
            _ => {
                invalid_instruction_called();
                None
            }
        },

        0xA => Some(i_Annn_LD_I_addr),
        0xB => Some(i_Bnnn_JP_V0_addr),
        0xC => Some(i_Cxkk_RND_Vx_byte),
        0xD => Some(i_Dxyn_DRW_Vx_Vy_nibble),

        0xE => match op.get_kk() {
            0x9E => Some(i_Ex9E_SKP_Vx),
            0xA1 => Some(i_ExA1_SKNP_Vx),
            _ => {
                invalid_instruction_called();
                None
            }
        },

        0xF => match op.get_kk() {
            0x07 => Some(i_Fx07_LD_Vx_DT),
            0x0A => Some(i_Fx0A_LD_Vx_K),
            0x15 => Some(i_Fx15_LD_DT_Vx),
            0x18 => Some(i_Fx18_LD_ST_Vx),
            0x1E => Some(i_Fx1E_ADD_I_Vx),
            0x29 => Some(i_Fx29_LD_F_Vx),
            0x33 => Some(i_Fx33_LD_B_Vx),
            0x55 => Some(i_Fx55_LD_I_Vx),
            0x65 => Some(i_Fx65_LD_Vx_I),
            _ => {
                invalid_instruction_called();
                None
            }
        },

        _ => panic!("op.get_s() should not be returning a byte > 0x0F"),
    }
}

fn invalid_instruction_called() {
    eprintln!("Invalid instruction called.");
}

#[allow(non_snake_case)]
fn i_00E0_CLS(this: &Cpu, _: &Opcode) -> bool {
    this.gpu.clear_framebuffer();
    false
}

#[allow(non_snake_case)]
fn i_00EE_RET(this: &Cpu, _op: &Opcode) -> bool {
    let Some(new_addr) = this.ram.pop_from_stack() else {
        return false;
    };

    this.set_pc(new_addr);
    false
}

#[allow(non_snake_case)]
fn i_1nnn_JP_addr(this: &Cpu, op: &Opcode) -> bool {
    this.set_pc(op.get_addr());
    false
}

#[allow(non_snake_case)]
fn i_2nnn_CALL_addr(this: &Cpu, op: &Opcode) -> bool {
    let mut pc = this.get_pc_ref();
    this.ram.push_to_stack(*pc);
    *pc = op.get_addr();
    false
}

#[allow(non_snake_case)]
fn i_3xkk_SE_Vx_byte(this: &Cpu, op: &Opcode) -> bool {
    if this.get_v_reg(op.get_x()) == op.get_kk() {
        this.increment_pc();
    }

    false
}

#[allow(non_snake_case)]
fn i_4xkk_SNE_Vx_byte(this: &Cpu, op: &Opcode) -> bool {
    if this.get_v_reg(op.get_x()) != op.get_kk() {
        this.increment_pc();
    }

    false
}

#[allow(non_snake_case)]
fn i_5xy0_SE_Vx_Vy(this: &Cpu, op: &Opcode) -> bool {
    let (vx, vy) = this.get_v_reg_xy(op.get_x(), op.get_y());

    if vx == vy {
        this.increment_pc();
    }

    false
}

#[allow(non_snake_case)]
fn i_6xkk_LD_Vx_byte(this: &Cpu, op: &Opcode) -> bool {
    this.set_v_reg(op.get_x(), op.get_kk());
    false
}

#[allow(non_snake_case)]
fn i_7xkk_ADD_Vx_byte(this: &Cpu, op: &Opcode) -> bool {
    let x = op.get_x_usize();
    let mut v = this.get_v_regs_ref();
    v[x] = v[x].wrapping_add(op.get_kk());
    false
}

#[allow(non_snake_case)]
fn i_8xy0_LD_Vx_Vy(this: &Cpu, op: &Opcode) -> bool {
    let mut v = this.get_v_regs_ref();
    v[op.get_x_usize()] = v[op.get_y_usize()];
    false
}

#[allow(non_snake_case)]
fn i_8xy1_OR_Vx_Vy(this: &Cpu, op: &Opcode) -> bool {
    let mut v = this.get_v_regs_ref();
    v[op.get_x_usize()] |= v[op.get_y_usize()];

    if this.config.reset_flag_for_bitwise_operations {
        v[0xF] = 0;
    }

    false
}

#[allow(non_snake_case)]
fn i_8xy2_AND_Vx_Vy(this: &Cpu, op: &Opcode) -> bool {
    let mut v = this.get_v_regs_ref();
    v[op.get_x_usize()] &= v[op.get_y_usize()];

    if this.config.reset_flag_for_bitwise_operations {
        v[0xF] = 0;
    }

    false
}

#[allow(non_snake_case)]
fn i_8xy3_XOR_Vx_Vy(this: &Cpu, op: &Opcode) -> bool {
    let mut v = this.get_v_regs_ref();
    v[op.get_x_usize()] ^= v[op.get_y_usize()];

    if this.config.reset_flag_for_bitwise_operations {
        v[0xF] = 0;
    }

    false
}

#[allow(non_snake_case)]
fn i_8xy4_ADD_Vx_Vy(this: &Cpu, op: &Opcode) -> bool {
    let (x, y) = op.get_x_and_y_usize();
    let mut v = this.get_v_regs_ref();
    let (val, wrapped) = v[x].overflowing_add(v[y]);
    v[x] = val;
    v[0xF] = u8::from(wrapped);
    false
}

#[allow(non_snake_case)]
fn i_8xy5_SUB_Vx_Vy(this: &Cpu, op: &Opcode) -> bool {
    let (x, y) = (op.get_x_usize(), op.get_y_usize());
    let mut v = this.get_v_regs_ref();
    let (val, wrapped) = v[x].overflowing_sub(v[y]);
    v[x] = val;
    v[0xF] = u8::from(!wrapped);
    false
}

#[allow(non_snake_case)]
fn i_8xy6_SHR_Vx(this: &Cpu, op: &Opcode) -> bool {
    let (x, y) = (op.get_x_usize(), op.get_y_usize());
    let mut v = this.get_v_regs_ref();

    let v_used = if this.config.use_new_shift_instruction {
        v[x]
    } else {
        v[y]
    };

    v[x] = v_used >> 1;
    v[0xF] = v_used & 1;
    false
}

#[allow(non_snake_case)]
fn i_8xy7_SUBN_Vx_Vy(this: &Cpu, op: &Opcode) -> bool {
    let (x, y) = (op.get_x_usize(), op.get_y_usize());
    let mut v = this.get_v_regs_ref();
    let (val, wrapped) = v[y].overflowing_sub(v[x]);
    v[x] = val;
    v[0xF] = u8::from(!wrapped);
    false
}

#[allow(non_snake_case)]
fn i_8xyE_SHL_Vx(this: &Cpu, op: &Opcode) -> bool {
    let (x, y) = (op.get_x_usize(), op.get_y_usize());
    let mut v = this.get_v_regs_ref();

    let v_used = if this.config.use_new_shift_instruction {
        v[x]
    } else {
        v[y]
    };

    v[x] = v_used << 1;
    v[0xF] = (v_used & 0x80) >> 7;
    false
}

#[allow(non_snake_case)]
fn i_9xy0_SNE_Vx_Vy(this: &Cpu, op: &Opcode) -> bool {
    let (x, y) = (op.get_x_usize(), op.get_y_usize());
    let v = this.get_v_regs_ref();

    if v[x] != v[y] {
        this.increment_pc();
    }

    false
}

#[allow(non_snake_case)]
fn i_Annn_LD_I_addr(this: &Cpu, op: &Opcode) -> bool {
    this.set_index_reg(op.get_addr());
    false
}

#[allow(non_snake_case)]
fn i_Bnnn_JP_V0_addr(this: &Cpu, op: &Opcode) -> bool {
    this.set_pc(if this.config.use_new_jump_instruction {
        u16::from(this.get_v_reg(op.get_x())) + op.get_addr()
    } else {
        u16::from(this.get_v_reg(0)) + op.get_addr()
    });
    false
}

#[allow(non_snake_case)]
fn i_Cxkk_RND_Vx_byte(this: &Cpu, op: &Opcode) -> bool {
    this.set_v_reg(op.get_x(), op.get_kk() & fastrand::u8(..));
    false
}

#[allow(non_snake_case)]
fn i_Dxyn_DRW_Vx_Vy_nibble(this: &Cpu, op: &Opcode) -> bool {
    let Some(sprite) = this
        .ram
        .read_bytes(this.get_index_reg(), u16::from(op.get_n()))
    else {
        return false;
    };

    let (x, y) = op.get_x_and_y_usize();
    let mut v = this.get_v_regs_ref();
    v[0xF] = u8::from(this.gpu.draw_sprite(&sprite, v[x], v[y]));

    if this.config.limit_to_one_draw_per_frame {
        this.gpu.wait_for_render();
        return true;
    }

    false
}

#[allow(non_snake_case)]
fn i_Ex9E_SKP_Vx(this: &Cpu, op: &Opcode) -> bool {
    if this.input_manager.get_key_state(this.get_v_reg(op.get_x())) {
        this.increment_pc();
    }

    false
}

#[allow(non_snake_case)]
fn i_ExA1_SKNP_Vx(this: &Cpu, op: &Opcode) -> bool {
    if !this.input_manager.get_key_state(this.get_v_reg(op.get_x())) {
        this.increment_pc();
    }

    false
}

#[allow(non_snake_case)]
fn i_Fx07_LD_Vx_DT(this: &Cpu, op: &Opcode) -> bool {
    this.set_v_reg(op.get_x(), this.delay_timer.get_value());
    false
}

#[allow(non_snake_case)]
fn i_Fx0A_LD_Vx_K(this: &Cpu, op: &Opcode) -> bool {
    this.set_v_reg(op.get_x(), this.input_manager.get_next_key_press());
    true
}

#[allow(non_snake_case)]
fn i_Fx15_LD_DT_Vx(this: &Cpu, op: &Opcode) -> bool {
    this.delay_timer.set_value(this.get_v_reg(op.get_x()));
    false
}

#[allow(non_snake_case)]
fn i_Fx18_LD_ST_Vx(this: &Cpu, op: &Opcode) -> bool {
    this.sound_timer.set_value(this.get_v_reg(op.get_x()));
    false
}

#[allow(non_snake_case)]
fn i_Fx1E_ADD_I_Vx(this: &Cpu, op: &Opcode) -> bool {
    let mut v = this.get_v_regs_ref();

    let Some(index_out_of_range) = this.increment_index_reg_by(u16::from(v[op.get_x_usize()]))
    else {
        return false;
    };

    if index_out_of_range && this.config.set_flag_for_index_overflow {
        v[0xF] = 1;
    }

    false
}

#[allow(non_snake_case)]
fn i_Fx29_LD_F_Vx(this: &Cpu, op: &Opcode) -> bool {
    debug_assert!(
        op.get_x() <= 0xF,
        "Should not be possible to query for two-character hex digits"
    );

    this.set_index_reg(this.ram.get_hex_digit_address(this.get_v_reg(op.get_x())));
    false
}

#[allow(non_snake_case)]
fn i_Fx33_LD_B_Vx(this: &Cpu, op: &Opcode) -> bool {
    let vx = this.get_v_reg(op.get_x());
    let bcd = vec![vx / 100, (vx / 10) % 10, vx % 10];
    this.ram.write_bytes(&bcd, this.get_index_reg());
    false
}

#[allow(non_snake_case)]
fn i_Fx55_LD_I_Vx(this: &Cpu, op: &Opcode) -> bool {
    let x = op.get_x();
    let index = this.get_index_reg_ref();

    this.ram
        .write_bytes(&this.get_v_reg_range(0..=x as usize), *index);

    if this.config.move_index_with_reads {
        this.increment_index_reg_ref_by(index, u16::from(x) + 1);
    }

    false
}

#[allow(non_snake_case)]
fn i_Fx65_LD_Vx_I(this: &Cpu, op: &Opcode) -> bool {
    let x = op.get_x();
    let index = this.get_index_reg_ref();

    let Some(bytes) = this.ram.read_bytes(*index, u16::from(x) + 1) else {
        return false;
    };

    this.set_v_reg_range(0, &bytes);

    if this.config.move_index_with_reads {
        this.increment_index_reg_ref_by(index, u16::from(x) + 1);
    }

    false
}
