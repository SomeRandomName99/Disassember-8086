use std::env;
use std::fmt::Write;
use std::fs;

const W_BIT_MASK: u8 = 0b1;
const S_BIT_SHIFT: u8 = 0b1;
const S_BIT_MASK: u8 = 0b1;
const REGISTER_MAP: [[&str; 2]; 8] = [
    ["al", "ax"],
    ["cl", "cx"],
    ["dl", "dx"],
    ["bl", "bx"],
    ["ah", "sp"],
    ["ch", "bp"],
    ["dh", "si"],
    ["bh", "di"],
];
const INSTRUCTION_NAMES: [&str; 8] = ["add", "or", "adc", "sbb", "and", "sub", "xor", "cmp"];
const CONDITIONAL_JMP_NAMES: [&str; 16] = [
    "jo", "jno", "jb", "jnb", "je", "jne", "jbe", "ja", "js", "jns", "jp", "jnp", "jl", "jge",
    "jle", "jg",
];
const LOOP_NAMES: [&str; 4] = ["loopnz", "loopz", "loop", "jcxz"];
const SEGMENT_REGS: [&str; 4] = ["es", "cs", "ss", "ds"];

fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = if args.len() != 2 {
        panic!("Usage: ./sim8086 path/to/binary/file");
    } else {
        args[1].parse::<String>().unwrap()
    };

    let instruction_stream = fs::read(file_path).expect("Could not read file");

    let mut arg1 = String::with_capacity(128);
    let mut arg2 = String::with_capacity(128);
    println!("bits 16");
    decode_instructions(&instruction_stream, &mut arg1, &mut arg2);
}

fn decode_instructions(mut bytes: &[u8], arg1: &mut String, arg2: &mut String) {
    // The while loop is needed because different instructions have different lengths
    'decode: while !bytes.is_empty() {
        // Clear the arena like strings
        arg1.clear();
        arg2.clear();

        let byte1 = bytes[0];

        // Match 4 bit instructions
        let opcode = byte1 >> 4;
        match opcode {
            0b1011 => {
                decode_mov_imm_reg(&mut bytes);
                continue 'decode;
            }
            _ => {}
        };

        // Match 5 bit instructions
        let opcode = byte1 >> 3;
        match opcode {
            0b01010 => {
                decode_push_pop_reg("push", &mut bytes);
                continue 'decode;
            }
            0b01011 => {
                decode_push_pop_reg("pop", &mut bytes);
                continue 'decode;
            }
            0b10010 => {
                decode_xchg_acc(&mut bytes);
                continue 'decode;
            }
            _ => {}
        };

        // Match 6 bit instructions
        let opcode = byte1 >> 2;
        match opcode {
            0b100000 => {
                decode_arithmetic_imm_regmem(&mut bytes, arg1, arg2);
                continue 'decode;
            }
            0b100010 => {
                decode_regmem_reg("mov", &mut bytes, arg1, arg2);
                continue 'decode;
            }
            0b000000 | 0b000010 | 0b000100 | 0b000110 | 0b001000 | 0b001010 | 0b001100
            | 0b001110 => {
                let inst_idx = (byte1 >> 3 & 0b111) as usize;
                let inst_name = INSTRUCTION_NAMES[inst_idx];
                decode_regmem_reg(inst_name, &mut bytes, arg1, arg2);
                continue 'decode;
            }
            _ => {}
        };

        // Match 7 bit instructions
        let opcode = byte1 >> 1;
        match opcode {
            0b1010000 => {
                decode_mov_mem_acc(&mut bytes, true);
                continue 'decode;
            }
            0b1010001 => {
                decode_mov_mem_acc(&mut bytes, false);
                continue 'decode;
            }
            0b1100011 => {
                decode_mov_imm_regmem(&mut bytes, arg1, arg2);
                continue 'decode;
            }
            0b0000010 | 0b0000110 | 0b0001010 | 0b0001110 | 0b0010010 | 0b0010110 | 0b0011010
            | 0b0011110 => {
                let inst_idx = (byte1 >> 3 & 0b0000111) as usize;
                let inst_name = INSTRUCTION_NAMES[inst_idx];
                decode_arithmetic_imm_acc(inst_name, &mut bytes);
                continue 'decode;
            }
            0b1000011 => {
                decode_regmem_reg("xchg", &mut bytes, arg1, arg2);
                continue 'decode;
            }
            _ => (),
        }

        // match 8 bit instruction
        match byte1 {
            0b01110000..=0b01111111 => {
                decode_jmp_and_loops(&mut bytes, true);
            }
            0b11100000..=0b11100011 => {
                decode_jmp_and_loops(&mut bytes, false);
            }
            0b11111111 => {
                decode_push_pop_mem("push", &mut bytes, arg1);
            }
            0b10001111 => {
                decode_push_pop_mem("pop", &mut bytes, arg1);
            }
            0b00000110 | 0b00001110 | 0b00010110 | 0b00011110 => {
                decode_push_pop_seg("push", &mut bytes);
            }
            0b00000111 | 0b00001111 | 0b00010111 | 0b00011111 => {
                decode_push_pop_seg("pop", &mut bytes);
            }
            _ => panic!(
                "Unsupported instruction. Opcode byte: {byte1:#b}, {:#b}",
                bytes[0]
            ),
        }
    }
}

enum EffectiveAddress {
    Reg(&'static str),                          // mod=0b11
    Direct(u16),                                // rm=0b110 mod=0
    Indirect { base: &'static str, disp: i16 }, // disp could be zero
}

// mod_bytes, rm, reg, displacement
fn decode_effective_address_calculation(
    bytes: &mut &[u8],
    w_bit: usize,
) -> (usize, EffectiveAddress) {
    const MOD_SHIFT: u8 = 6;
    const RM_MASK: u8 = 0b000000111;
    const REG_SHIFT: u8 = 3;
    const REG_MASK: u8 = 0b00111000;

    let byte = bytes[0];
    *bytes = &bytes[1..];
    let mod_bytes = byte >> MOD_SHIFT;
    let r_m = (byte & RM_MASK) as usize;
    let reg = ((byte & REG_MASK) >> REG_SHIFT) as usize;

    if mod_bytes == 0b11 {
        (reg, EffectiveAddress::Reg(REGISTER_MAP[r_m][w_bit]))
    } else {
        // Direct address mode
        if r_m == 0b110 && mod_bytes == 0 {
            let address: u16 = u16::from_le_bytes([bytes[0], bytes[1]]);
            *bytes = &bytes[2..];

            (reg, EffectiveAddress::Direct(address))
        } else {
            let rm_reg_str = match r_m {
                0b000 => "bx + si",
                0b001 => "bx + di",
                0b010 => "bp + si",
                0b011 => "bp + di",
                0b100 => "si",
                0b101 => "di",
                0b110 => "bp",
                0b111 => "bx",
                _ => panic!("Invalid R/M"),
            };
            let mut displacement: i16 = 0;
            if mod_bytes == 0b01 {
                displacement = (bytes[0] as i8) as i16; // sign extend to 16 bits
                *bytes = &bytes[1..];
            } else if mod_bytes == 0b10 {
                displacement = i16::from_le_bytes([bytes[0], bytes[1]]);
                *bytes = &bytes[2..];
            }
            (
                reg,
                EffectiveAddress::Indirect {
                    base: rm_reg_str,
                    disp: displacement,
                },
            )
        }
    }
}

fn write_effective_address(buffer: &mut String, eff_add: EffectiveAddress) {
    match eff_add {
        EffectiveAddress::Reg(rm_reg_str) => {
            buffer.push_str(rm_reg_str);
        }
        EffectiveAddress::Direct(address) => {
            write!(buffer, "[{address}]").unwrap();
        }
        EffectiveAddress::Indirect { base, disp } => {
            if disp > 0 {
                write!(buffer, "[{} + {}]", base, disp).unwrap();
            } else if disp < 0 {
                write!(buffer, "[{} - {}]", base, disp.unsigned_abs()).unwrap();
            } else {
                write!(buffer, "[{}]", base).unwrap();
            }
        }
    }
}

fn decode_mov_imm_regmem(bytes: &mut &[u8], dst: &mut String, src: &mut String) {
    let byte1 = bytes[0];
    *bytes = &bytes[1..];
    let w_bit = (byte1 & W_BIT_MASK) as usize;

    let (_, eff_add) = decode_effective_address_calculation(bytes, w_bit);
    if !matches!(eff_add, EffectiveAddress::Reg(_)) {
        if w_bit == 1 {
            dst.push_str("word ");
        } else {
            dst.push_str("byte ");
        }
    }
    write_effective_address(dst, eff_add);

    if w_bit == 1 {
        let immediate = i16::from_le_bytes([bytes[0], bytes[1]]);
        *bytes = &bytes[2..];
        write!(src, "{immediate}").unwrap();
    } else {
        let immediate = (bytes[0] as i8) as i16; // sign extend to 16 bits
        *bytes = &bytes[1..];
        write!(src, "{immediate}").unwrap();
    }

    println!("mov {dst}, {src}");
}

fn decode_regmem_reg(instruction: &str, bytes: &mut &[u8], arg1: &mut String, arg2: &mut String) {
    const D_BIT_SHIFT: u8 = 1;
    const D_BIT_MASK: u8 = 0b00000010;

    let byte1 = bytes[0];
    *bytes = &bytes[1..];

    let w_bit = (byte1 & W_BIT_MASK) as usize;
    let d_bit: bool = matches!((byte1 & D_BIT_MASK) >> D_BIT_SHIFT, 1);

    let (dst, src) = match d_bit {
        true => (&mut *arg1, &mut *arg2),
        false => (&mut *arg2, &mut *arg1),
    };

    let (reg, eff_add) = decode_effective_address_calculation(bytes, w_bit);
    dst.push_str(REGISTER_MAP[reg][w_bit]);
    write_effective_address(src, eff_add);

    println!("{instruction} {arg1}, {arg2}");
}

fn decode_mov_imm_reg(bytes: &mut &[u8]) {
    // This is the only instruction with the w bit not at the end of the opcode byte
    const W_BIT_MASK: u8 = 0b00001000;
    const W_BIT_SHIFT: u8 = 3;
    const REG_MASK: u8 = 0b00000111;

    let byte1 = bytes[0];
    *bytes = &bytes[1..];
    let w_bit = ((byte1 & W_BIT_MASK) >> W_BIT_SHIFT) as usize;
    let reg = (byte1 & REG_MASK) as usize;
    let reg_str: &str = REGISTER_MAP[reg][w_bit];

    let mut immediate: i16 = 0;
    if w_bit == 1 {
        immediate = i16::from_le_bytes([bytes[0], bytes[1]]);
        *bytes = &bytes[2..];
    } else {
        immediate |= (bytes[0] as i8) as i16; // sign extend to 16 bits
        *bytes = &bytes[1..];
    }

    println!("mov {}, {}", reg_str, immediate);
}

fn decode_mov_mem_acc(bytes: &mut &[u8], acc_first: bool) {
    let address = u16::from_le_bytes([bytes[1], bytes[2]]);
    *bytes = &bytes[3..];

    if acc_first {
        println!("mov ax, [{address}]");
    } else {
        println!("mov [{address}], ax");
    }
}

fn decode_arithmetic_imm_regmem(bytes: &mut &[u8], dst: &mut String, src: &mut String) {
    let byte1 = bytes[0];
    *bytes = &bytes[1..];

    let w_bit = (byte1 & W_BIT_MASK) as usize;
    let s_bit = (byte1 >> S_BIT_SHIFT) & S_BIT_MASK;

    let (inst_idx, eff_add) = decode_effective_address_calculation(bytes, w_bit);

    let inst_name = INSTRUCTION_NAMES[inst_idx];
    if !matches!(eff_add, EffectiveAddress::Reg(_)) {
        if w_bit == 1 {
            dst.push_str("word ");
        } else {
            dst.push_str("byte ");
        }
    }
    write_effective_address(dst, eff_add);

    if w_bit == 0 {
        let immediate = bytes[0] as i8;
        write!(src, "{immediate}").unwrap();
        *bytes = &bytes[1..];
    } else if s_bit == 0 && w_bit == 1 {
        let immediate = i16::from_le_bytes([bytes[0], bytes[1]]);
        write!(src, "{immediate}").unwrap();
        *bytes = &bytes[2..];
    } else {
        let immediate = bytes[0] as i8 as i16; // sign extend to 16 bits
        write!(src, "{immediate}").unwrap();
        *bytes = &bytes[1..];
    }

    println!("{inst_name} {dst}, {src}");
}

fn decode_arithmetic_imm_acc(inst_name: &str, bytes: &mut &[u8]) {
    let byte1 = bytes[0];
    *bytes = &bytes[1..];

    let w_bit = (byte1 & W_BIT_MASK) as usize;

    let mut immediate: i16 = 0;
    if w_bit == 1 {
        immediate = i16::from_le_bytes([bytes[0], bytes[1]]);
        *bytes = &bytes[2..];
    } else {
        immediate |= (bytes[0] as i8) as i16; // sign extend to 16 bits
        *bytes = &bytes[1..];
    }
    let acc_name = if w_bit == 1 { "ax" } else { "al" };

    println!("{inst_name} {acc_name}, {immediate}");
}

fn decode_jmp_and_loops(bytes: &mut &[u8], is_jmp: bool) {
    let byte1 = bytes[0];
    *bytes = &bytes[1..];

    let inst_name = if is_jmp {
        CONDITIONAL_JMP_NAMES[(byte1 & 0b1111) as usize]
    } else {
        LOOP_NAMES[(byte1 & 0b11) as usize]
    };

    let disp = bytes[0] as i8;
    *bytes = &bytes[1..];

    println!("{inst_name} $+2+{disp}");
}

fn decode_push_pop_mem(inst_name: &str, bytes: &mut &[u8], dst: &mut String) {
    *bytes = &bytes[1..];

    dst.push_str("word ");
    let (_, eff_add) = decode_effective_address_calculation(bytes, 1);
    write_effective_address(dst, eff_add);

    println!("{inst_name} {dst}");
}

fn decode_push_pop_reg(inst_name: &str, bytes: &mut &[u8]) {
    let byte1 = bytes[0];
    *bytes = &bytes[1..];

    let reg_idx = (byte1 & 0b111) as usize;

    println!("{inst_name} {}", REGISTER_MAP[reg_idx][1]);
}

fn decode_push_pop_seg(inst_name: &str, bytes: &mut &[u8]) {
    let byte1 = bytes[0];
    *bytes = &bytes[1..];

    let seg_idx = ((byte1 >> 3) & 0b11) as usize;

    println!("{inst_name} {}", SEGMENT_REGS[seg_idx]);
}

fn decode_xchg_acc(bytes: &mut &[u8]) {
    let byte1 = bytes[0];
    *bytes = &bytes[1..];

    let reg_idx = (byte1 & 0b111) as usize;
    println!("xchg ax, {}", REGISTER_MAP[reg_idx][1]);
}
