use std::env;
use std::fmt::Write;
use std::fs;

const W_BIT_MASK: u8 = 0b1;
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

        // Match 6 bit instructions
        let opcode = byte1 >> 2;
        match opcode {
            0b100010 => {
                decode_mov_regmem_reg(&mut bytes, arg1, arg2);
                continue 'decode;
            }
            _ => {}
        };

        // Match 7 bit instructions
        let opcode = byte1 >> 1;
        match opcode {
            0b1010000 => {
                decode_mov_mem_accu(&mut bytes, true);
                continue 'decode;
            }
            0b1010001 => {
                decode_mov_mem_accu(&mut bytes, false);
                continue 'decode;
            }
            0b1100011 => {
                decode_mov_imm_regmem(&mut bytes, arg1, arg2);
                continue 'decode;
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
) -> (&'static str, EffectiveAddress) {
    const MOD_SHIFT: u8 = 6;
    const RM_MASK: u8 = 0b000000111;
    const REG_SHIFT: u8 = 3;
    const REG_MASK: u8 = 0b00111000;

    let byte = bytes[0];
    *bytes = &bytes[1..];
    let mod_bytes = byte >> MOD_SHIFT;
    let r_m = (byte & RM_MASK) as usize;
    let reg = ((byte & REG_MASK) >> REG_SHIFT) as usize;
    let reg_str: &str = REGISTER_MAP[reg][w_bit];

    if mod_bytes == 0b11 {
        (reg_str, EffectiveAddress::Reg(REGISTER_MAP[r_m][w_bit]))
    } else {
        // Direct address mode
        if r_m == 0b110 && mod_bytes == 0 {
            let address: u16 = u16::from_le_bytes([bytes[0], bytes[1]]);
            *bytes = &bytes[2..];

            (reg_str, EffectiveAddress::Direct(address))
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
                displacement = (bytes[0] as i8) as i16;
                *bytes = &bytes[1..];
            } else if mod_bytes == 0b10 {
                displacement = i16::from_le_bytes([bytes[0], bytes[1]]);
                *bytes = &bytes[2..];
            }
            (
                reg_str,
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
    write_effective_address(dst, eff_add);

    if w_bit == 1 {
        let immediate = i16::from_le_bytes([bytes[0], bytes[1]]);
        *bytes = &bytes[2..];
        write!(src, "word {immediate}").unwrap();
    } else {
        let immediate = (bytes[0] as i8) as i16;
        *bytes = &bytes[1..];
        write!(src, "byte {immediate}").unwrap();
    }

    println!("mov {dst} {src}");
}

fn decode_mov_regmem_reg(bytes: &mut &[u8], arg1: &mut String, arg2: &mut String) {
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

    let (reg_str, eff_add) = decode_effective_address_calculation(bytes, w_bit);
    dst.push_str(reg_str);
    write_effective_address(src, eff_add);

    println!("mov {}, {}", arg1, arg2);
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
        immediate |= (bytes[0] as i8) as i16;
        *bytes = &bytes[1..];
    }

    println!("mov {}, {}", reg_str, immediate);
}

fn decode_mov_mem_accu(bytes: &mut &[u8], accu_first: bool) {
    let address = u16::from_le_bytes([bytes[1], bytes[2]]);
    *bytes = &bytes[3..];

    if accu_first {
        println!("mov ax, [{address}]");
    } else {
        println!("mov [{address}], ax");
    }
}
