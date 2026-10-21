use crate::cpu::Cpu;
use alloc::vec;
use log::error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Opcode {
    full: u16,
}

impl Opcode {
    pub fn from_u8s(high: u8, low: u8) -> Self {
        Self {
            full: (u16::from(high) << 8) | u16::from(low),
        }
    }

    pub const fn addr(self) -> u16 {
        self.full & 0x0FFF
    }

    pub const fn kk(self) -> u8 {
        (self.full & 0x00FF) as u8
    }

    pub const fn s(self) -> u8 {
        ((self.full & 0xF000) >> 12) as u8
    }

    pub const fn x(self) -> u8 {
        ((self.full & 0x0F00) >> 8) as u8
    }

    pub const fn x_usize(self) -> usize {
        self.x() as usize
    }

    pub const fn y(self) -> u8 {
        ((self.full & 0x00F0) >> 4) as u8
    }

    pub const fn y_usize(self) -> usize {
        self.y() as usize
    }

    pub const fn x_and_y_usize(self) -> (usize, usize) {
        (self.x_usize(), self.y_usize())
    }

    pub const fn n(self) -> u8 {
        (self.full & 0x000F) as u8
    }
}

pub type InstructionFunction = fn(&Cpu, Opcode) -> bool;

pub fn instruction_function(op: Opcode) -> Option<InstructionFunction> {
    match op.s() {
        0x0 => match op.addr() {
            0x0E0 => Some(i_00e0_cls),
            0x0EE => Some(i_00ee_ret),
            _ => {
                error!("Machine code routines are not supported.");
                None
            }
        },

        0x1 => Some(i_1nnn_jp_addr),
        0x2 => Some(i_2nnn_call_addr),
        0x3 => Some(i_3xkk_se_vx_byte),
        0x4 => Some(i_4xkk_sne_vx_byte),

        0x5 => {
            if op.n() == 0x0 {
                Some(i_5xy0_se_vx_vy)
            } else {
                invalid_instruction_called();
                None
            }
        }

        0x6 => Some(i_6xkk_ld_vx_byte),
        0x7 => Some(i_7xkk_add_vx_byte),

        0x8 => match op.n() {
            0x0 => Some(i_8xy0_ld_vx_vy),
            0x1 => Some(i_8xy1_or_vx_vy),
            0x2 => Some(i_8xy2_and_vx_vy),
            0x3 => Some(i_8xy3_xor_vx_vy),
            0x4 => Some(i_8xy4_add_vx_vy),
            0x5 => Some(i_8xy5_sub_vx_vy),
            0x6 => Some(i_8xy6_shr_vx),
            0x7 => Some(i_8xy7_subn_vx_vy),
            0xE => Some(i_8xye_shl_vx),
            _ => {
                invalid_instruction_called();
                None
            }
        },

        0x9 => {
            if op.n() == 0x0 {
                Some(i_9xy0_sne_vx_vy)
            } else {
                invalid_instruction_called();
                None
            }
        }

        0xA => Some(i_annn_ld_i_addr),
        0xB => Some(i_bnnn_jp_v0_addr),
        0xC => Some(i_cxkk_rnd_vx_byte),
        0xD => Some(i_dxyn_drw_vx_vy_nibble),

        0xE => match op.kk() {
            0x9E => Some(i_ex9e_skp_vx),
            0xA1 => Some(i_exa1_sknp_vx),
            _ => {
                invalid_instruction_called();
                None
            }
        },

        0xF => match op.kk() {
            0x07 => Some(i_fx07_ld_vx_dt),
            0x0A => Some(i_fx0a_ld_vx_k),
            0x15 => Some(i_fx15_ld_dt_vx),
            0x18 => Some(i_fx18_ld_st_vx),
            0x1E => Some(i_fx1e_add_i_vx),
            0x29 => Some(i_fx29_ld_f_vx),
            0x33 => Some(i_fx33_ld_b_vx),
            0x55 => Some(i_fx55_ld_i_vx),
            0x65 => Some(i_fx65_ld_vx_i),
            _ => {
                invalid_instruction_called();
                None
            }
        },

        _ => panic!("op.s() should not be returning a byte > 0x0F"),
    }
}

fn invalid_instruction_called() {
    error!("Invalid instruction called.");
}

fn i_00e0_cls(this: &Cpu, _: Opcode) -> bool {
    this.gpu.clear_framebuffer();
    false
}

fn i_00ee_ret(this: &Cpu, _op: Opcode) -> bool {
    let Some(new_addr) = this.ram.pop_from_stack() else {
        return false;
    };

    this.set_pc(new_addr);
    false
}

fn i_1nnn_jp_addr(this: &Cpu, op: Opcode) -> bool {
    this.set_pc(op.addr());
    false
}

fn i_2nnn_call_addr(this: &Cpu, op: Opcode) -> bool {
    let mut pc = this.pc_ref();
    this.ram.push_to_stack(*pc);
    *pc = op.addr();
    false
}

fn i_3xkk_se_vx_byte(this: &Cpu, op: Opcode) -> bool {
    if this.v_reg(op.x()) == op.kk() {
        this.increment_pc();
    }

    false
}

fn i_4xkk_sne_vx_byte(this: &Cpu, op: Opcode) -> bool {
    if this.v_reg(op.x()) != op.kk() {
        this.increment_pc();
    }

    false
}

fn i_5xy0_se_vx_vy(this: &Cpu, op: Opcode) -> bool {
    let (vx, vy) = this.v_reg_xy(op.x(), op.y());

    if vx == vy {
        this.increment_pc();
    }

    false
}

fn i_6xkk_ld_vx_byte(this: &Cpu, op: Opcode) -> bool {
    this.set_v_reg(op.x(), op.kk());
    false
}

fn i_7xkk_add_vx_byte(this: &Cpu, op: Opcode) -> bool {
    let x = op.x_usize();
    let mut v = this.v_regs_ref();
    v[x] = v[x].wrapping_add(op.kk());
    false
}

fn i_8xy0_ld_vx_vy(this: &Cpu, op: Opcode) -> bool {
    let mut v = this.v_regs_ref();
    v[op.x_usize()] = v[op.y_usize()];
    false
}

fn i_8xy1_or_vx_vy(this: &Cpu, op: Opcode) -> bool {
    let mut v = this.v_regs_ref();
    v[op.x_usize()] |= v[op.y_usize()];

    if this.config.reset_flag_for_bitwise_operations {
        v[0xF] = 0;
    }

    false
}

fn i_8xy2_and_vx_vy(this: &Cpu, op: Opcode) -> bool {
    let mut v = this.v_regs_ref();
    v[op.x_usize()] &= v[op.y_usize()];

    if this.config.reset_flag_for_bitwise_operations {
        v[0xF] = 0;
    }

    false
}

fn i_8xy3_xor_vx_vy(this: &Cpu, op: Opcode) -> bool {
    let mut v = this.v_regs_ref();
    v[op.x_usize()] ^= v[op.y_usize()];

    if this.config.reset_flag_for_bitwise_operations {
        v[0xF] = 0;
    }

    false
}

fn i_8xy4_add_vx_vy(this: &Cpu, op: Opcode) -> bool {
    let (x, y) = op.x_and_y_usize();
    let mut v = this.v_regs_ref();
    let (val, wrapped) = v[x].overflowing_add(v[y]);
    v[x] = val;
    v[0xF] = wrapped.into();
    false
}

fn i_8xy5_sub_vx_vy(this: &Cpu, op: Opcode) -> bool {
    let (x, y) = (op.x_usize(), op.y_usize());
    let mut v = this.v_regs_ref();
    let (val, wrapped) = v[x].overflowing_sub(v[y]);
    v[x] = val;
    v[0xF] = (!wrapped).into();
    false
}

fn i_8xy6_shr_vx(this: &Cpu, op: Opcode) -> bool {
    let (x, y) = (op.x_usize(), op.y_usize());
    let mut v = this.v_regs_ref();

    let v_used = if this.config.use_new_shift_instruction {
        v[x]
    } else {
        v[y]
    };

    v[x] = v_used >> 1;
    v[0xF] = v_used & 1;
    false
}

fn i_8xy7_subn_vx_vy(this: &Cpu, op: Opcode) -> bool {
    let (x, y) = (op.x_usize(), op.y_usize());
    let mut v = this.v_regs_ref();
    let (val, wrapped) = v[y].overflowing_sub(v[x]);
    v[x] = val;
    v[0xF] = (!wrapped).into();
    false
}

fn i_8xye_shl_vx(this: &Cpu, op: Opcode) -> bool {
    let (x, y) = (op.x_usize(), op.y_usize());
    let mut v = this.v_regs_ref();

    let v_used = if this.config.use_new_shift_instruction {
        v[x]
    } else {
        v[y]
    };

    v[x] = v_used << 1;
    v[0xF] = (v_used & 0x80) >> 7;
    false
}

fn i_9xy0_sne_vx_vy(this: &Cpu, op: Opcode) -> bool {
    let (x, y) = (op.x_usize(), op.y_usize());
    let v = this.v_regs_ref();

    if v[x] != v[y] {
        this.increment_pc();
    }

    drop(v);

    false
}

fn i_annn_ld_i_addr(this: &Cpu, op: Opcode) -> bool {
    this.set_index_reg(op.addr());
    false
}

fn i_bnnn_jp_v0_addr(this: &Cpu, op: Opcode) -> bool {
    this.set_pc(if this.config.use_new_jump_instruction {
        u16::from(this.v_reg(op.x())) + op.addr()
    } else {
        u16::from(this.v_reg(0)) + op.addr()
    });
    false
}

fn i_cxkk_rnd_vx_byte(this: &Cpu, op: Opcode) -> bool {
    this.set_v_reg(op.x(), op.kk() & fastrand::u8(..));
    false
}

fn i_dxyn_drw_vx_vy_nibble(this: &Cpu, op: Opcode) -> bool {
    let Some(sprite) = this.ram.read_bytes(this.index_reg(), op.n().into()) else {
        return false;
    };

    let (x, y) = op.x_and_y_usize();
    let mut v = this.v_regs_ref();
    v[0xF] = this.gpu.draw_sprite(&sprite, v[x], v[y]).into();
    drop(v);

    if this.config.limit_to_one_draw_per_frame {
        this.gpu.wait_for_render();
        return true;
    }

    false
}

fn i_ex9e_skp_vx(this: &Cpu, op: Opcode) -> bool {
    if this.input_manager.key_state(this.v_reg(op.x())) {
        this.increment_pc();
    }

    false
}

fn i_exa1_sknp_vx(this: &Cpu, op: Opcode) -> bool {
    if !this.input_manager.key_state(this.v_reg(op.x())) {
        this.increment_pc();
    }

    false
}

fn i_fx07_ld_vx_dt(this: &Cpu, op: Opcode) -> bool {
    this.set_v_reg(op.x(), this.delay_timer.value());
    false
}

fn i_fx0a_ld_vx_k(this: &Cpu, op: Opcode) -> bool {
    this.set_v_reg(op.x(), this.input_manager.next_key_press());
    true
}

fn i_fx15_ld_dt_vx(this: &Cpu, op: Opcode) -> bool {
    this.delay_timer.set_value(this.v_reg(op.x()));
    false
}

fn i_fx18_ld_st_vx(this: &Cpu, op: Opcode) -> bool {
    this.sound_timer.set_value(this.v_reg(op.x()));
    false
}

fn i_fx1e_add_i_vx(this: &Cpu, op: Opcode) -> bool {
    let mut v = this.v_regs_ref();

    let Some(index_out_of_range) = this.increment_index_reg_by(v[op.x_usize()].into()) else {
        return false;
    };

    if index_out_of_range && this.config.set_flag_for_index_overflow {
        v[0xF] = 1;
    }

    false
}

fn i_fx29_ld_f_vx(this: &Cpu, op: Opcode) -> bool {
    assert!(
        op.x() > 0xF,
        "Should not be possible to query for two-character hex digits"
    );

    this.set_index_reg(this.ram.hex_digit_address(this.v_reg(op.x())));
    false
}

fn i_fx33_ld_b_vx(this: &Cpu, op: Opcode) -> bool {
    let vx = this.v_reg(op.x());
    let bcd = vec![vx / 100, (vx / 10) % 10, vx % 10];
    this.ram.write_bytes(&bcd, this.index_reg());
    false
}

fn i_fx55_ld_i_vx(this: &Cpu, op: Opcode) -> bool {
    let x = op.x();
    let index = this.index_reg_ref();

    this.ram
        .write_bytes(&this.v_reg_range(0..=x as usize), *index);

    if this.config.move_index_with_reads {
        this.increment_index_reg_ref_by(index, u16::from(x) + 1);
    }

    false
}

fn i_fx65_ld_vx_i(this: &Cpu, op: Opcode) -> bool {
    let x = op.x();
    let index = this.index_reg_ref();

    let Some(bytes) = this.ram.read_bytes(*index, u16::from(x) + 1) else {
        return false;
    };

    this.set_v_reg_range(0, &bytes);

    if this.config.move_index_with_reads {
        this.increment_index_reg_ref_by(index, u16::from(x) + 1);
    }

    false
}
