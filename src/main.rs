use std::env;
use std::fmt::Write;
use std::fs;

const OPCODE_SHIFT: u8 = 2;

const D_BIT_SHIFT: u8 = 1;
const D_BIT_MASK: u8 = 0b00000010;

const W_BIT_SHIFT: u8 = 0;
const W_BIT_MASK: u8 = 0b00000001;

const MOD_SHIFT: u8 = 6;

const REG_SHIFT: u8 = 3;
const REG_MASK: u8 = 0b00111000;

const RM_MASK: u8 = 0b000000111;

const MOV_OPCODE: u8 = 0b100010;

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
    while !bytes.is_empty() {
        // Clear the arena like strings
        arg1.clear();
        arg2.clear();

        let byte1 = bytes[0];
        bytes = &bytes[1..];

        let opcode = byte1 >> OPCODE_SHIFT;
        let inst = match opcode {
            MOV_OPCODE => "mov",
            _ => panic!("Unsupported instruction. Opcode: {opcode}"),
        };

        let w_bit = ((byte1 & W_BIT_MASK) >> W_BIT_SHIFT) as usize;
        let d_bit: bool = matches!((byte1 & D_BIT_MASK) >> D_BIT_SHIFT, 1);

        if bytes.is_empty() {
            panic!("Not enough bytes to decode instructions");
        }
        let byte2 = bytes[0];
        bytes = &bytes[1..];
        let mod_bytes = byte2 >> MOD_SHIFT;

        let reg = ((byte2 & REG_MASK) >> REG_SHIFT) as usize;
        let reg_arg: &str = REGISTER_MAP[reg][w_bit];
        let r_m = (byte2 & RM_MASK) as usize;

        let (dst, src) = match d_bit {
            true => (&mut *arg1, &mut *arg2),
            false => (&mut *arg2, &mut *arg1),
        };
        if mod_bytes == 0b11 {
            let rm_arg: &str = REGISTER_MAP[r_m][w_bit];

            dst.push_str(reg_arg);
            src.push_str(rm_arg);
        } else {
            assert!(
                !(r_m == 0b110 && mod_bytes == 0),
                "Direct address mode is not yet supported"
            );
            let reg = match r_m {
                0b000 => "BX + SI",
                0b001 => "BX + DI",
                0b010 => "BP + SI",
                0b011 => "BP + DI",
                0b100 => "SI",
                0b101 => "DI",
                0b110 => "BP",
                0b111 => "BX",
                _ => panic!("Invalid R/M"),
            };
            let mut displacement: u16 = 0;
            if mod_bytes == 0b01 {
                displacement |= bytes[0] as u16;
                bytes = &bytes[1..];
            } else if mod_bytes == 0b10 {
                displacement |= (bytes[0] as u16) | ((bytes[1] as u16) << 8);
                bytes = &bytes[2..];
            }
            write!(dst, "{}", reg_arg).unwrap();
            if mod_bytes != 0 {
                write!(src, "[{} + {}]", reg, displacement).unwrap();
            } else {
                write!(src, "[{}]", reg).unwrap();
            }
        }

        println!("{} {}, {}", inst, arg1, arg2);
    }
}
