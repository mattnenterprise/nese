use mapper;
use controller;
use apu;
use ppu;

use std::rc::Rc;
use std::cell::RefCell;

pub trait Memory {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, cycle: u64, addr: u16, data: u8);
    fn get_added_stall(&mut self) -> u32;
}

pub struct CPU<T: Memory> {
    mem: T,
    // Accumulator
    a: u8,
    x: u8,
    y: u8,
    // Program Counter
    pc: u16,
    // Stack Pointer
    sp: u8,
    instruction_num: u32,
    pub stall: u32,
    cycles: u64,
    carry_flag: bool,
    zero_flag: bool,
    interrupt_disable_flag: bool,
    decimal_mode_flag: bool,
    unused_bit4_flag: bool,
    unused_bit5_flag: bool,
    overflow_flag: bool,
    negative_flag: bool,
    trigger_irq: bool,
}

impl<T: Memory> CPU<T> {
    pub fn new(mem: T) -> CPU<T> {
        let mut cpu = CPU{
            mem: mem,
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            sp: 0xFD,
            instruction_num: 0,
            stall: 0,
            cycles: 0,
            carry_flag: false,
            zero_flag: false,
            interrupt_disable_flag: true,
            decimal_mode_flag: false,
            unused_bit4_flag: false,
            unused_bit5_flag: true,
            overflow_flag: false,
            negative_flag: false,
            trigger_irq: false,
        };
        cpu.pc = cpu.read16(0xFFFC);
        // TODO this is just for testing with nestext.nes the real initial address to load the pc is above this.
        //cpu.pc = 0xC000;
        cpu
    }

    pub fn step(&mut self, nmi: bool) -> u64 {
        self.stall += self.mem.get_added_stall();
        if self.stall > 0 {
            self.stall -=1;
            return 1;
        }

        let cycles: u64 = self.cycles;
        if nmi {
            self.nmi();
            self.cycles += 7;
        } else if self.trigger_irq {
            self.irq();
            self.cycles += 7;
            self.trigger_irq = false;
        }

        let opcode = self.mem.read(self.pc);
        let addressing_mode = self.instruction_addressing_mode(opcode);
        let _flags = self.get_flags();
        self.instruction_num += 1;
        let mut step_info = StepInfo{
            opcode: opcode,
            address: 0,
            addressing_mode: addressing_mode,

        };

        let mut page_crossed = false;
        match step_info.addressing_mode {
            AddressingMode::Absolute => {
                let read_address = self.pc + 1;
                step_info.address = self.read16(read_address);
            },
            AddressingMode::AbsoluteX => {
                let read_address = self.pc + 1;
                step_info.address = self.read16(read_address).wrapping_add(self.x as u16);
                page_crossed = read_address&0xFF00 != step_info.address&0xFF00;
            },
            AddressingMode::AbsoluteY => {
                let read_address = self.pc + 1;
                step_info.address = self.read16(read_address).wrapping_add(self.y as u16);
                page_crossed = read_address&0xFF00 != step_info.address&0xFF00;
            },
            AddressingMode::Accumulator => {},
            AddressingMode::IndexedIndirect => {
                let address = self.mem.read(self.pc + 1).wrapping_add(self.x);
                step_info.address = self.read16_zero_page(address);
            },
            AddressingMode::Indirect => {
                let pc = self.pc;
                let address = self.read16(pc + 1);
                step_info.address = self.read16_low_byte_wrap(address);
            }
            AddressingMode::IndirectIndexed => {
                let address = self.mem.read(self.pc + 1);
                step_info.address = self.read16_zero_page(address).wrapping_add(self.y as u16);
                page_crossed = (address as u16)&0xFF00 != step_info.address&0xFF00;
            },
            AddressingMode::Immediate => {
                step_info.address = self.pc + 1;
            },
            AddressingMode::Implied => {},
            AddressingMode::Relative => {
                let offset_address = self.pc + 1;
                let offset = self.mem.read(offset_address);

                if offset < 0x80 {
                    step_info.address = self.pc + 2 + offset as u16;
                } else {
                    step_info.address = self.pc + 2 + offset as u16 - 0x100;
                }
            },
            AddressingMode::Unknown => panic!("Unknown addressing mode for opcode {:#04X}", opcode),
            AddressingMode::ZeroPage => {
                let read_address = self.pc + 1;
                step_info.address = self.mem.read(read_address) as u16;
            },
            AddressingMode::ZeroPageX => {
                let address = self.pc + 1;
                step_info.address = self.mem.read(address).wrapping_add(self.x) as u16;
            },
            AddressingMode::ZeroPageY => {
                let address = self.pc + 1;
                step_info.address = self.mem.read(address).wrapping_add(self.y) as u16;
            }
        };


        let _instruction_size = self.instruction_size(opcode);
        //println!("Instruction Num: {:06} , size: {:06} , OpCode: {:#06X} , Address: {:#06X} , PC: {:#06X} , A: {:#04X} , X: {:#04X} , Y: {:#04X} , P: {:#04X}, SP: {:#04X}", self.instruction_num, _instruction_size, opcode, step_info.address, self.pc, self.a, self.x, self.y, _flags, self.sp);

        self.pc += self.instruction_size(opcode);
        self.cycles += self.instruction_cycles(opcode) as u64;
        if page_crossed {
            self.cycles += self.page_crossed_cycles(opcode) as u64;
        }
        
        self.run_instruction(step_info);

        return self.cycles - cycles;
    }

    fn add_branch_cycles(&mut self, step_info: StepInfo) {
        self.cycles += 1;
        if self.pc&0xFF00 != step_info.address&0xFF00 {
            self.cycles += 1;
        }
    }

    fn instruction_addressing_mode(&mut self, opcode: u8) -> AddressingMode {
        match opcode {
            0x00 => AddressingMode::Immediate,
            0x01 => AddressingMode::IndexedIndirect,
            0x04 => AddressingMode::ZeroPage,
            0x05 => AddressingMode::ZeroPage,
            0x06 => AddressingMode::ZeroPage,
            0x08 => AddressingMode::Implied,
            0x09 => AddressingMode::Immediate,
            0x0A => AddressingMode::Accumulator,
            0x0D => AddressingMode::Absolute,
            0x0E => AddressingMode::Absolute,
            0x10 => AddressingMode::Relative,
            0x11 => AddressingMode::IndirectIndexed,
            0x15 => AddressingMode::ZeroPageX,
            0x16 => AddressingMode::ZeroPageX,
            0x18 => AddressingMode::Implied,
            0x19 => AddressingMode::AbsoluteY,
            0x1D => AddressingMode::AbsoluteX,
            0x1E => AddressingMode::AbsoluteX,
            0x20 => AddressingMode::Absolute,
            0x21 => AddressingMode::IndexedIndirect,
            0x24 => AddressingMode::ZeroPage,
            0x25 => AddressingMode::ZeroPage,
            0x26 => AddressingMode::ZeroPage,
            0x28 => AddressingMode::Implied,
            0x29 => AddressingMode::Immediate,
            0x2A => AddressingMode::Accumulator,
            0x2C => AddressingMode::Absolute,
            0x2D => AddressingMode::Absolute,
            0x2E => AddressingMode::Absolute,
            0x30 => AddressingMode::Relative,
            0x31 => AddressingMode::IndirectIndexed,
            0x35 => AddressingMode::ZeroPageX,
            0x36 => AddressingMode::ZeroPageX,
            0x38 => AddressingMode::Implied,
            0x39 => AddressingMode::AbsoluteY,
            0x3D => AddressingMode::AbsoluteX,
            0x3E => AddressingMode::AbsoluteX,
            0x40 => AddressingMode::Implied,
            0x41 => AddressingMode::IndexedIndirect,
            0x44 => AddressingMode::ZeroPage,
            0x45 => AddressingMode::ZeroPage,
            0x46 => AddressingMode::ZeroPage,
            0x48 => AddressingMode::Implied,
            0x49 => AddressingMode::Immediate,
            0x4A => AddressingMode::Accumulator,
            0x4C => AddressingMode::Absolute,
            0x4D => AddressingMode::Absolute,
            0x4E => AddressingMode::Absolute,
            0x50 => AddressingMode::Relative,
            0x51 => AddressingMode::IndirectIndexed,
            0x55 => AddressingMode::ZeroPageX,
            0x56 => AddressingMode::ZeroPageX,
            0x58 => AddressingMode::Implied,
            0x59 => AddressingMode::AbsoluteY,
            0x5D => AddressingMode::AbsoluteX,
            0x5E => AddressingMode::AbsoluteX,
            0x60 => AddressingMode::Implied,
            0x61 => AddressingMode::IndexedIndirect,
            0x65 => AddressingMode::ZeroPage,
            0x66 => AddressingMode::ZeroPage,
            0x68 => AddressingMode::Implied,
            0x69 => AddressingMode::Immediate,
            0x6A => AddressingMode::Accumulator,
            0x6C => AddressingMode::Indirect,
            0x6E => AddressingMode::Absolute,
            0x6D => AddressingMode::Absolute,
            0x70 => AddressingMode::Relative,
            0x71 => AddressingMode::IndirectIndexed,
            0x75 => AddressingMode::ZeroPageX,
            0x76 => AddressingMode::ZeroPageX,
            0x78 => AddressingMode::Implied,
            0x79 => AddressingMode::AbsoluteY,
            0x7D => AddressingMode::AbsoluteX,
            0x7E => AddressingMode::AbsoluteX,
            0x81 => AddressingMode::IndexedIndirect,
            0x84 => AddressingMode::ZeroPage,
            0x85 => AddressingMode::ZeroPage,
            0x86 => AddressingMode::ZeroPage,
            0x88 => AddressingMode::Implied,
            0x8A => AddressingMode::Implied,
            0x8C => AddressingMode::Absolute,
            0x8D => AddressingMode::Absolute,
            0x8E => AddressingMode::Absolute,
            0x90 => AddressingMode::Relative,
            0x91 => AddressingMode::IndirectIndexed,
            0x94 => AddressingMode::ZeroPageX,
            0x95 => AddressingMode::ZeroPageX,
            0x96 => AddressingMode::ZeroPageY,
            0x98 => AddressingMode::Implied,
            0x99 => AddressingMode::AbsoluteY,
            0x9A => AddressingMode::Implied,
            0x9D => AddressingMode::AbsoluteX,
            0xA0 => AddressingMode::Immediate,
            0xA1 => AddressingMode::IndexedIndirect,
            0xA2 => AddressingMode::Immediate,
            0xA4 => AddressingMode::ZeroPage,
            0xA5 => AddressingMode::ZeroPage,
            0xA6 => AddressingMode::ZeroPage,
            0xA8 => AddressingMode::Implied,
            0xA9 => AddressingMode::Immediate,
            0xAA => AddressingMode::Implied,
            0xAC => AddressingMode::Absolute,
            0xAD => AddressingMode::Absolute,
            0xAE => AddressingMode::Absolute,
            0xB0 => AddressingMode::Relative,
            0xB1 => AddressingMode::IndirectIndexed,
            0xB4 => AddressingMode::ZeroPageX,
            0xB5 => AddressingMode::ZeroPageX,
            0xB6 => AddressingMode::ZeroPageY,
            0xB8 => AddressingMode::Implied,
            0xB9 => AddressingMode::AbsoluteY,
            0xBA => AddressingMode::Implied,
            0xBC => AddressingMode::AbsoluteX,
            0xBD => AddressingMode::AbsoluteX,
            0xBE => AddressingMode::AbsoluteY,
            0xC0 => AddressingMode::Immediate,
            0xC1 => AddressingMode::IndexedIndirect,
            0xC4 => AddressingMode::ZeroPage,
            0xC5 => AddressingMode::ZeroPage,
            0xC6 => AddressingMode::ZeroPage,
            0xC8 => AddressingMode::Implied,
            0xC9 => AddressingMode::Immediate,
            0xCA => AddressingMode::Implied,
            0xCC => AddressingMode::Absolute,
            0xCD => AddressingMode::Absolute,
            0xCE => AddressingMode::Absolute,
            0xD0 => AddressingMode::Relative,
            0xD1 => AddressingMode::IndirectIndexed,
            0xD5 => AddressingMode::ZeroPageX,
            0xD6 => AddressingMode::ZeroPageX,
            0xD8 => AddressingMode::Implied,
            0xD9 => AddressingMode::AbsoluteY,
            0xDD => AddressingMode::AbsoluteX,
            0xDE => AddressingMode::AbsoluteX,
            0xE0 => AddressingMode::Immediate,
            0xE1 => AddressingMode::IndexedIndirect,
            0xE4 => AddressingMode::ZeroPage,
            0xE5 => AddressingMode::ZeroPage,
            0xE6 => AddressingMode::ZeroPage,
            0xE8 => AddressingMode::Implied,
            0xE9 => AddressingMode::Immediate,
            0xEA => AddressingMode::Implied,
            0xEC => AddressingMode::Absolute,
            0xED => AddressingMode::Absolute,
            0xEE => AddressingMode::Absolute,
            0xF0 => AddressingMode::Relative,
            0xF1 => AddressingMode::IndirectIndexed,
            0xF5 => AddressingMode::ZeroPageX,
            0xF6 => AddressingMode::ZeroPageX,
            0xF8 => AddressingMode::Implied,
            0xF9 => AddressingMode::AbsoluteY,
            0xFD => AddressingMode::AbsoluteX,
            0xFE => AddressingMode::AbsoluteX,
            _ => AddressingMode::Unknown,
        }
    }

    fn instruction_cycles(&mut self, opcode: u8) -> u16 {
        match opcode {
            0x00 => 7,
            0x01 => 6,
            0x05 => 3,
            0x06 => 5,
            0x08 => 3,
            0x09 => 2,
            0x0A => 2,
            0x0D => 4,
            0x0E => 6,
            0x10 => 2,
            0x11 => 5,
            0x15 => 4,
            0x16 => 6,
            0x18 => 2,
            0x19 => 4,
            0x1D => 4,
            0x1E => 7, 
            0x20 => 6,
            0x21 => 6,
            0x24 => 3,
            0x25 => 3,
            0x26 => 5,
            0x28 => 4,
            0x29 => 2,
            0x2A => 2,
            0x2C => 4,
            0x2D => 4,
            0x2E => 6,
            0x30 => 2,
            0x31 => 5,
            0x35 => 4,
            0x36 => 6,
            0x38 => 2,
            0x39 => 4,
            0x3D => 4,
            0x3E => 7,
            0x40 => 6,
            0x41 => 6,
            0x45 => 3,
            0x46 => 5,
            0x48 => 3,
            0x49 => 2,
            0x4A => 2,
            0x4C => 3,
            0x4D => 4,
            0x4E => 6,
            0x50 => 2,
            0x51 => 5,
            0x55 => 4,
            0x56 => 6,
            0x58 => 2,
            0x59 => 4,
            0x5D => 4,
            0x5E => 7,
            0x60 => 6,
            0x61 => 6,
            0x65 => 3,
            0x66 => 5,
            0x68 => 4,
            0x69 => 2,
            0x6A => 2,
            0x6C => 5,
            0x6D => 4,
            0x6E => 6,
            0x70 => 2,
            0x71 => 5,
            0x75 => 4,
            0x76 => 6,
            0x78 => 2,
            0x79 => 4,
            0x7D => 4,
            0x7E => 7,
            0x81 => 6,
            0x84 => 3,
            0x85 => 3,
            0x86 => 3,
            0x88 => 2,
            0x8A => 2,
            0x8C => 4,
            0x8D => 4,
            0x8E => 4,
            0x90 => 2,
            0x91 => 6,
            0x94 => 4,
            0x95 => 4,
            0x96 => 4,
            0x98 => 2,
            0x99 => 5,
            0x9A => 2,
            0x9D => 5,
            0xA0 => 2,
            0xA1 => 6,
            0xA2 => 2,
            0xA4 => 3,
            0xA5 => 3,
            0xA6 => 3,
            0xA8 => 2,
            0xA9 => 2,
            0xAA => 2,
            0xAC => 4,
            0xAD => 4,
            0xAE => 4,
            0xB0 => 2,
            0xB1 => 5,
            0xB4 => 4,
            0xB5 => 4,
            0xB6 => 4,
            0xB8 => 2,
            0xB9 => 4,
            0xBA => 2,
            0xBC => 4,
            0xBD => 4,
            0xBE => 4,
            0xC0 => 2,
            0xC1 => 6,
            0xC4 => 3,
            0xC5 => 3,
            0xC6 => 5,
            0xC8 => 2,
            0xC9 => 2,
            0xCA => 2,
            0xCC => 4, 
            0xCD => 4,
            0xCE => 6,
            0xD0 => 2,
            0xD1 => 5,
            0xD5 => 4,
            0xD6 => 6,
            0xD8 => 2,
            0xD9 => 4,
            0xDD => 4,
            0xDE => 7,
            0xE0 => 2,
            0xE1 => 6,
            0xE4 => 3,
            0xE5 => 3,
            0xE6 => 5,
            0xE8 => 2,
            0xE9 => 2,
            0xEA => 2,
            0xEC => 4,
            0xED => 4,
            0xEE => 6,
            0xF0 => 2,
            0xF1 => 5,
            0xF5 => 4,
            0xF6 => 6,
            0xF8 => 2,
            0xF9 => 4,
            0xFD => 4,
            0xFE => 7,
            _ => panic!("Unknown instruction cycles for opcode {:#04X}", opcode),
        }
    }

    // number of additional cycles used when a page is crossed.
    fn page_crossed_cycles(&mut self, opcode: u8) -> u16 {
        match opcode {
            0x00 => 0,
            0x01 => 0,
            0x05 => 0,
            0x06 => 0,
            0x08 => 0,
            0x09 => 0,
            0x0A => 0,
            0x0D => 0,
            0x0E => 0,
            0x11 => 1,
            0x15 => 0,
            0x16 => 0,
            0x18 => 0,
            0x19 => 1,
            0x1D => 1,
            0x1E => 0,
            0x21 => 0,
            0x24 => 0,
            0x25 => 0,
            0x29 => 0,
            0x2C => 0,
            0x2D => 0,
            0x31 => 1,
            0x35 => 0,
            0x39 => 1,
            0x3D => 1,
            0x3E => 0,
            0x40 => 0,
            0x41 => 0,
            0x45 => 0,
            0x51 => 1,
            0x59 => 1,
            0x5D => 1,
            0x5E => 0, 
            0x61 => 0,
            0x65 => 0,
            0x69 => 0,
            0x6D => 0,
            0x71 => 1,
            0x75 => 0,
            0x79 => 1,
            0x7D => 1,
            0x7E => 0,
            0x81 => 0,
            0x91 => 0,
            0x99 => 0,
            0x9D => 0,
            0xA0 => 0,
            0xA1 => 0,
            0xA2 => 0,
            0xA4 => 0,
            0xB1 => 1,
            0xB9 => 1,
            0xBC => 1,
            0xBD => 1,
            0xBE => 1,
            0xD1 => 1,
            0xD9 => 1,
            0xDD => 1,
            0xDE => 0,
            0xEC => 0,
            0xED => 0,
            0xEE => 0,
            0xF1 => 1,
            0xF5 => 0,
            0xF6 => 0,
            0xF8 => 0,
            0xF9 => 1,
            0xFD => 1,
            0xFE => 0,
            _ => panic!("Unknown page crossed cycles for opcode {:#04X}", opcode),
        }
    }

    fn instruction_size(&mut self, opcode: u8) -> u16 {
        match opcode {
            0x00 => 2,
            0x01 => 2,
            0x04 => 2,
            0x05 => 2,
            0x06 => 2,
            0x08 => 1,
            0x09 => 2,
            0x0A => 1,
            0x0D => 3,
            0x0E => 3,
            0x10 => 2,
            0x11 => 2,
            0x15 => 2,
            0x16 => 2,
            0x18 => 1,
            0x19 => 3,
            0x1D => 3,
            0x1E => 3,
            0x20 => 3,
            0x21 => 2,
            0x24 => 2,
            0x25 => 2,
            0x26 => 2,
            0x28 => 1,
            0x29 => 2,
            0x2A => 1,
            0x2C => 3,
            0x2D => 3,
            0x2E => 3,
            0x30 => 2,
            0x31 => 2,
            0x35 => 2,
            0x36 => 2,
            0x38 => 1,
            0x39 => 3,
            0x3D => 3,
            0x3E => 3,
            0x40 => 1,
            0x41 => 2,
            0x44 => 2,
            0x45 => 2,
            0x46 => 2,
            0x48 => 1,
            0x49 => 2,
            0x4A => 1,
            0x4C => 3,
            0x4D => 3,
            0x4E => 3,
            0x50 => 2,
            0x51 => 2,
            0x55 => 2,
            0x56 => 2,
            0x58 => 1,
            0x59 => 3,
            0x5D => 3,
            0x5E => 3,
            0x60 => 1,
            0x61 => 2,
            0x65 => 2,
            0x66 => 2,
            0x68 => 1,
            0x69 => 2,
            0x6A => 1,
            0x6C => 3,
            0x6D => 3,
            0x6E => 3,
            0x70 => 2,
            0x71 => 2,
            0x75 => 2,
            0x76 => 2,
            0x78 => 1,
            0x79 => 3,
            0x7D => 3,
            0x7E => 3,
            0x81 => 2,
            0x84 => 2,
            0x85 => 2,
            0x86 => 2,
            0x88 => 1,
            0x8A => 1,
            0x8C => 3,
            0x8D => 3,
            0x8E => 3,
            0x90 => 2,
            0x91 => 2,
            0x94 => 2,
            0x95 => 2,
            0x96 => 2,
            0x98 => 1,
            0x99 => 3,
            0x9A => 1,
            0x9D => 3,
            0xA0 => 2,
            0xA1 => 2,
            0xA2 => 2,
            0xA4 => 2,
            0xA5 => 2,
            0xA6 => 2,
            0xA8 => 1,
            0xA9 => 2,
            0xAA => 1,
            0xAC => 3,
            0xAD => 3,
            0xAE => 3,
            0xB0 => 2,
            0xB1 => 2,
            0xB4 => 2,
            0xB5 => 2,
            0xB6 => 2,
            0xB8 => 1,
            0xB9 => 3,
            0xBA => 1,
            0xBC => 3,
            0xBD => 3,
            0xBE => 3,
            0xC0 => 2,
            0xC1 => 2,
            0xC4 => 2,
            0xC5 => 2,
            0xC6 => 2,
            0xC8 => 1,
            0xC9 => 2,
            0xCA => 1,
            0xCC => 3,
            0xCD => 3,
            0xCE => 3,
            0xD0 => 2,
            0xD1 => 2,
            0xD5 => 2,
            0xD6 => 2,
            0xD8 => 1,
            0xD9 => 3,
            0xDD => 3,
            0xDE => 3,
            0xE0 => 2,
            0xE1 => 2,
            0xE4 => 2,
            0xE5 => 2,
            0xE6 => 2,
            0xE8 => 1,
            0xE9 => 2,
            0xEA => 1,
            0xEC => 3,
            0xED => 3,
            0xEE => 3,
            0xF0 => 2,
            0xF1 => 2,
            0xF5 => 2,
            0xF6 => 2,
            0xF8 => 1,
            0xF9 => 3,
            0xFD => 3,
            0xFE => 3,
            _ => panic!("Unknown instruction size for opcode {:#04X}", opcode),
        }
    }

    fn run_instruction(&mut self, step_info: StepInfo) {
        match step_info.opcode {
            0x00 => self.brk(step_info),
            0x01 => self.ora(step_info),
            0x04 => self.nop(step_info),
            0x05 => self.ora(step_info),
            0x06 => self.asl(step_info),
            0x08 => self.php(step_info),
            0x09 => self.ora(step_info),
            0x0A => self.asl(step_info),
            0x0D => self.ora(step_info),
            0x0E => self.asl(step_info),
            0x10 => self.bpl(step_info),
            0x11 => self.ora(step_info),
            0x15 => self.ora(step_info),
            0x16 => self.asl(step_info),
            0x18 => self.clc(step_info),
            0x19 => self.ora(step_info),
            0x1D => self.ora(step_info),
            0x1E => self.asl(step_info),
            0x20 => self.jsr(step_info),
            0x21 => self.and(step_info),
            0x24 => self.bit(step_info),
            0x25 => self.and(step_info),
            0x26 => self.rol(step_info),
            0x28 => self.plp(step_info),
            0x29 => self.and(step_info),
            0x2A => self.rol(step_info),
            0x2C => self.bit(step_info),
            0x2D => self.and(step_info),
            0x2E => self.rol(step_info),
            0x30 => self.bmi(step_info),
            0x31 => self.and(step_info),
            0x35 => self.and(step_info),
            0x36 => self.rol(step_info),
            0x38 => self.sec(step_info),
            0x39 => self.and(step_info),
            0x3D => self.and(step_info),
            0x3E => self.rol(step_info),
            0x40 => self.rti(step_info),
            0x41 => self.eor(step_info),
            0x44 => self.nop(step_info),
            0x45 => self.eor(step_info),
            0x46 => self.lsr(step_info),
            0x48 => self.pha(step_info),
            0x49 => self.eor(step_info),
            0x4A => self.lsr(step_info),
            0x4C => self.jmp(step_info),
            0x4D => self.eor(step_info),
            0x4E => self.lsr(step_info),
            0x50 => self.bvc(step_info),
            0x51 => self.eor(step_info),
            0x55 => self.eor(step_info),
            0x56 => self.lsr(step_info),
            0x58 => self.cli(step_info),
            0x59 => self.eor(step_info),
            0x5D => self.eor(step_info),
            0x5E => self.lsr(step_info),
            0x60 => self.rts(step_info),
            0x61 => self.adc(step_info),
            0x65 => self.adc(step_info),
            0x66 => self.ror(step_info),
            0x68 => self.pla(step_info),
            0x69 => self.adc(step_info),
            0x6A => self.ror(step_info),
            0x6C => self.jmp(step_info),
            0x6D => self.adc(step_info),
            0x6E => self.ror(step_info),
            0x70 => self.bvs(step_info),
            0x71 => self.adc(step_info),
            0x75 => self.adc(step_info),
            0x76 => self.ror(step_info),
            0x78 => self.sei(step_info),
            0x79 => self.adc(step_info),
            0x7D => self.adc(step_info),
            0x7E => self.ror(step_info),
            0x81 => self.sta(step_info),
            0x84 => self.sty(step_info),
            0x85 => self.sta(step_info),
            0x86 => self.stx(step_info),
            0x88 => self.dey(step_info),
            0x8A => self.txa(step_info),
            0x8C => self.sty(step_info),
            0x8D => self.sta(step_info),
            0x8E => self.stx(step_info),
            0x90 => self.bcc(step_info),
            0x91 => self.sta(step_info),
            0x94 => self.sty(step_info),
            0x95 => self.sta(step_info),
            0x96 => self.stx(step_info),
            0x98 => self.tya(step_info),
            0x99 => self.sta(step_info),
            0x9A => self.txs(step_info),
            0x9D => self.sta(step_info),
            0xA0 => self.ldy(step_info),
            0xA1 => self.lda(step_info),
            0xA2 => self.ldx(step_info),
            0xA4 => self.ldy(step_info),
            0xA5 => self.lda(step_info),
            0xA6 => self.ldx(step_info),
            0xA8 => self.tay(step_info),
            0xA9 => self.lda(step_info),
            0xAC => self.ldy(step_info),
            0xAA => self.tax(step_info),
            0xAD => self.lda(step_info),
            0xAE => self.ldx(step_info),
            0xB0 => self.bcs(step_info),
            0xB1 => self.lda(step_info),
            0xB4 => self.ldy(step_info),
            0xB5 => self.lda(step_info),
            0xB6 => self.ldx(step_info),
            0xB8 => self.clv(step_info),
            0xB9 => self.lda(step_info),
            0xBA => self.tsx(step_info),
            0xBC => self.ldy(step_info),
            0xBD => self.lda(step_info),
            0xBE => self.ldx(step_info),
            0xC0 => self.cpy(step_info),
            0xC1 => self.cmp(step_info),
            0xC4 => self.cpy(step_info),
            0xC5 => self.cmp(step_info),
            0xC6 => self.dec(step_info),
            0xC8 => self.iny(step_info),
            0xC9 => self.cmp(step_info),
            0xCA => self.dex(step_info),
            0xCC => self.cpy(step_info),
            0xCD => self.cmp(step_info),
            0xCE => self.dec(step_info),
            0xD0 => self.bne(step_info),
            0xD1 => self.cmp(step_info),
            0xD5 => self.cmp(step_info),
            0xD6 => self.dec(step_info),
            0xD8 => self.cld(step_info),
            0xD9 => self.cmp(step_info),
            0xDD => self.cmp(step_info),
            0xDE => self.dec(step_info),
            0xE0 => self.cpx(step_info),
            0xE1 => self.sbc(step_info),
            0xE4 => self.cpx(step_info),
            0xE5 => self.sbc(step_info),
            0xE6 => self.inc(step_info),
            0xE8 => self.inx(step_info),
            0xE9 => self.sbc(step_info),
            0xEA => self.nop(step_info),
            0xEC => self.cpx(step_info),
            0xED => self.sbc(step_info),
            0xEE => self.inc(step_info),
            0xF0 => self.beq(step_info),
            0xF1 => self.sbc(step_info),
            0xF5 => self.sbc(step_info),
            0xF6 => self.inc(step_info),
            0xF8 => self.sed(step_info),
            0xF9 => self.sbc(step_info),
            0xFD => self.sbc(step_info),
            0xFE => self.inc(step_info),
            _ => self.unimplemented(step_info),
        }
    }

    fn nmi(&mut self) {
        let pc = self.pc;
        self.push16(pc);
        self.php(StepInfo{
            opcode: 0,
            address: 0,
            addressing_mode: AddressingMode::Unknown,
        });
        self.pc = self.read16(0xFFFA);
        self.interrupt_disable_flag = true;
    }

    pub fn set_irq(&mut self) {
        if !self.interrupt_disable_flag {
            self.trigger_irq = true;
        }
    }

    fn irq(&mut self) {
        let pc = self.pc;
        self.push16(pc);
        self.php(StepInfo{
            opcode: 0,
            address: 0,
            addressing_mode: AddressingMode::Unknown,
        });
        self.pc = self.read16(0xFFFE);
        self.interrupt_disable_flag = true;
    }

    fn set_negative(&mut self, value: u8) {
        if value&0x80 != 0 {
            self.negative_flag = true;
        } else {
            self.negative_flag = false;
        }
    }

    fn set_zero(&mut self, value: u8) {
        if value == 0 {
            self.zero_flag = true;
        } else {
            self.zero_flag = false;
        }
    }

    //  Add with Carry
    fn adc(&mut self, step_info: StepInfo) {
        let data = self.mem.read(step_info.address);
        let result: u32 = (self.a as u32) + (data as u32) + (self.carry_flag as u32);

        if result > 0xFF {
            self.carry_flag = true;
        } else {
            self.carry_flag = false;
        }

        let a = self.a;
        let result = result as u8;
        if (a^data)&0x80 == 0 && (a^result)&0x80 != 0 {
            self.overflow_flag = true;
        } else {
            self.overflow_flag = false;
        }
        self.a = result;
        self.set_negative(result);
        self.set_zero(result);
    }

    fn and(&mut self, step_info: StepInfo) {
        let data = self.mem.read(step_info.address);
        self.a = self.a & data;
        let a = self.a;
        self.set_negative(a);
        self.set_zero(a);
    }

    // Arithmetic Shift Left
    fn asl(&mut self, step_info: StepInfo) {
        match step_info.addressing_mode {
            AddressingMode::Accumulator => {
                if self.a & 0x80 == 0x80 {
                    self.carry_flag = true;
                } else {
                    self.carry_flag = false;
                }
                self.a = self.a << 1;
                let a = self.a;
                self.set_zero(a);
                self.set_negative(a);
            },
            _ => {
                let mut data = self.mem.read(step_info.address);
                if data & 0x80 == 0x80 {
                    self.carry_flag = true;
                } else {
                    self.carry_flag = false;
                }
                data = data << 1;
                self.mem.write(self.cycles, step_info.address, data);
                self.set_zero(data);
                self.set_negative(data);
             }
        }
    }

    // Branch on Carry Clear
    fn bcc(&mut self, step_info: StepInfo) {
        if !self.carry_flag {
            self.add_branch_cycles(step_info.clone());
            self.pc = step_info.address;
        }
    }

    // Branch on Carry Set
    fn bcs(&mut self, step_info: StepInfo) {
        if self.carry_flag {
            self.add_branch_cycles(step_info.clone());
            self.pc = step_info.address;
        }
    }

    // BEQ - Branch if Equal
    // If the zero flag is set then add the relative displacement to the program counter to cause a branch to a new location.
    fn beq(&mut self, step_info: StepInfo) {
        if self.zero_flag {
            self.add_branch_cycles(step_info.clone());
            self.pc = step_info.address;
        }
    }

    // test BITs
    fn bit(&mut self, step_info: StepInfo) {
        let value = self.mem.read(step_info.address);

        if (value >> 6) & 1 == 1 {
            self.overflow_flag = true;
        } else {
            self.overflow_flag = false;
        }

        let a = self.a;
        self.set_zero(value & a);
        self.set_negative(value);
    }

    // Branch if Minus
    fn bmi(&mut self, step_info: StepInfo) {
        if self.negative_flag {
            self.add_branch_cycles(step_info.clone());
            self.pc = step_info.address;
        }
    }

    // BNE - Branch if Not Equal
    // If the zero flag is clear then add the relative displacement to the program counter to cause a branch to a new location.
    fn bne(&mut self, step_info: StepInfo) {
        if !self.zero_flag {
            self.add_branch_cycles(step_info.clone());
            self.pc = step_info.address;
        }
    }

    // Branch if Positive
    fn bpl(&mut self, step_info: StepInfo) {
        if !self.negative_flag {
            self.add_branch_cycles(step_info.clone());
            self.pc = step_info.address;
        }
    }

    // Break
    fn brk(&mut self, step_info: StepInfo) {
        let pc = self.pc;
        self.push16(pc);
        self.php(step_info);
        self.sei(StepInfo{
            opcode: 0,
            address: 0,
            addressing_mode: AddressingMode::Immediate,
        });
        self.pc = self.read16(0xFFFE);
    }

    // Branch if Overflow Clear
    fn bvc(&mut self, step_info: StepInfo) {
        if !self.overflow_flag {
            self.add_branch_cycles(step_info.clone());
            self.pc = step_info.address;
        }
    }

    // Branch on oVerflow Set
    fn bvs(&mut self, step_info: StepInfo) {
        if self.overflow_flag {
            self.add_branch_cycles(step_info.clone());
            self.pc = step_info.address;
        }
    }

    // Clear Carry Flag
    fn clc(&mut self, _: StepInfo) {
        self.carry_flag = false;
    }

    // Clear Overflow Flag
    fn clv(&mut self, _: StepInfo) {
        self.overflow_flag = false;
    }

    // Compare X Register
    fn cpx(&mut self, step_info: StepInfo) {
        let data = self.mem.read(step_info.address);
        let result: i32 = (self.x as i32) - (data as i32);


        self.set_negative(result as u8);
        self.set_zero(result as u8);

        if self.x >= data {
            self.carry_flag = true;
        } else {
            self.carry_flag = false;
        }
    }

    // Compare Y Register
    fn cpy(&mut self, step_info: StepInfo) {
        let data = self.mem.read(step_info.address);
        let result: i32 = (self.y as i32) - (data as i32);


        self.set_negative(result as u8);
        self.set_zero(result as u8);

        if self.y >= data {
            self.carry_flag = true;
        } else {
            self.carry_flag = false;
        }
    }

    // CLear Interrupt
    fn cli(&mut self, _: StepInfo) {
        self.interrupt_disable_flag = false;
    }

    // CoMPare accumulator
    fn cmp(&mut self, step_info: StepInfo) {
        let data = self.mem.read(step_info.address);
        let result: i32 = (self.a as i32) - (data as i32);


        self.set_negative(result as u8);
        self.set_zero(result as u8);

        if self.a >= data {
            self.carry_flag = true;
        } else {
            self.carry_flag = false;
        }
    }

    // Decrement Memory
    fn dec(&mut self, step_info: StepInfo) {
        let mut data = self.mem.read(step_info.address);
        data = data.wrapping_sub(1);
        self.set_negative(data);
        self.set_zero(data);
        self.mem.write(self.cycles, step_info.address, data);
    }

    // Decrement X
    fn dex(&mut self, _: StepInfo) {
        let new_x = self.x as i32 - 1;

        self.x = new_x as u8;
        let x = self.x;
        self.set_negative(x);
        self.set_zero(x);
    }

    // Decrement Y
    fn dey(&mut self, _: StepInfo) {
        let new_y = self.y as i32 - 1;

        self.y = new_y as u8;
        let y = self.y;
        self.set_negative(y);
        self.set_zero(y);
    }

    // Exclusive OR
    fn eor(&mut self, step_info: StepInfo) {
        let data = self.mem.read(step_info.address);
        self.a = self.a ^ data;

        let a = self.a;
        self.set_negative(a);
        self.set_zero(a);
    }

    // Increment Memory
    fn inc(&mut self, step_info: StepInfo) {
        let mut data = self.mem.read(step_info.address);
        data = data.wrapping_add(1);
        self.set_negative(data);
        self.set_zero(data);
        self.mem.write(self.cycles, step_info.address, data);
    }

    // Increment X Register
    fn inx(&mut self, _: StepInfo) {
        let new_x = self.x as u32 + 1;

        self.x = new_x as u8;
        let x = self.x;
        self.set_negative(x);
        self.set_zero(x);
    }

    // Increment Y Register
    fn iny(&mut self, _: StepInfo) {
        let new_y = self.y as u32 + 1;

        self.y = new_y as u8;
        let y = self.y;
        self.set_negative(y);
        self.set_zero(y);
    }

     // Subtract with Carry
    fn sbc(&mut self, step_info: StepInfo) {
        let data = self.mem.read(step_info.address);
        let result: i32 = (self.a as i32) - (data as i32) - (1 - (self.carry_flag as i32));
        let resultu8: u8 =  (self.a as u8).wrapping_sub(data as u8).wrapping_sub(1 - (self.carry_flag as u8));

        if result >= 0 {
            self.carry_flag = true;
        } else {
            self.carry_flag = false;
        }

        let a = self.a;
        //let result = result as u8;
        if (a^data)&0x80 != 0 && (a^resultu8)&0x80 != 0 {
            self.overflow_flag = true;
        } else {
            self.overflow_flag = false;
        }
        self.a = resultu8;
        self.set_negative(resultu8);
        self.set_zero(resultu8);
    }

    // Set Carry Flag
    fn sec(&mut self, _: StepInfo) {
        self.carry_flag = true;
    }

    // Set Interrupt Disable Flag
    fn sei(&mut self, _: StepInfo) {
        self.interrupt_disable_flag = true;
    }

    // Transfer Accumulator to X Index
    fn tax(&mut self, _: StepInfo) {
        let a = self.a;
        self.set_negative(a);
        self.set_zero(a);
        self.x = a;
    }

    // Transfer Accumulator to Y Index
    fn tay(&mut self, _: StepInfo) {
        let a = self.a;
        self.set_negative(a);
        self.set_zero(a);
        self.y = a;
    }

    // Transfer Stack Pointer to X
    fn tsx(&mut self, _: StepInfo) {
        self.x = self.sp;
        let x = self.x;
        self.set_negative(x);
        self.set_zero(x);
    }

    // Transfer X to Accumulator
    fn txa(&mut self, _: StepInfo) {
        let x = self.x;
        self.set_negative(x);
        self.set_zero(x);
        self.a = x;
    }

    // Transfer Y to Accumulator
    fn tya(&mut self, _: StepInfo) {
        let y = self.y;
        self.set_negative(y);
        self.set_zero(y);
        self.a = y;
    }

    // Clear Decimal Flag
    fn cld(&mut self, _: StepInfo) {
        self.decimal_mode_flag = false;
    }

    // Load Accumulator With Memory
    fn lda(&mut self, step_info: StepInfo) {
        let value = self.mem.read(step_info.address);
        self.set_negative(value);
        self.set_zero(value);
        self.a = value;
    }

    // Load X Index With Memory
    fn ldx(&mut self, step_info: StepInfo) {
        let value = self.mem.read(step_info.address);
        self.x = value;

        self.set_negative(value);
        self.set_zero(value);
    }

    // Load Y Index With Memory
    fn ldy(&mut self, step_info: StepInfo) {
        let value = self.mem.read(step_info.address);
        self.y = value;

        self.set_negative(value);
        self.set_zero(value);
    }

    // Logical Shift Right
    fn lsr(&mut self, step_info: StepInfo) {
        match step_info.addressing_mode {
            AddressingMode::Accumulator => {
                if self.a & 1 == 1 {
                    self.carry_flag = true;
                } else {
                    self.carry_flag = false;
                }
                self.a = self.a >> 1;
                let a = self.a;
                self.set_zero(a);
                self.set_negative(a);
            },
            _ => {
                let mut data = self.mem.read(step_info.address);
                if data & 1 == 1 {
                    self.carry_flag = true;
                } else {
                    self.carry_flag = false;
                }
                data = data >> 1;
                self.mem.write(self.cycles, step_info.address, data);
                self.set_zero(data);
                self.set_negative(data);
             }
        }
    }

    // Nop does nothing
    fn nop(&mut self, _: StepInfo) {

    }

    // Logical Inclusive OR
    fn ora(&mut self, step_info: StepInfo) {
        let data = self.mem.read(step_info.address);
        self.a = self.a | data;

        let a = self.a;
        self.set_negative(a);
        self.set_zero(a);
    }

    // Push Accumulator
    fn pha(&mut self, _: StepInfo) {
        let a = self.a;
        self.push(a);
    }

    // Push Processor Status
    fn php(&mut self, _: StepInfo) {
        let flags = self.get_flags();
        self.push(flags);
    }

    // Pull Accumulator
    fn pla(&mut self, _: StepInfo) {
        self.a = self.pop();
        let a = self.a;
        self.set_negative(a);
        self.set_zero(a);
    }

    // Pull Processor Status
    fn plp(&mut self, _: StepInfo) {
        let flags = self.pop();
        self.set_flags(flags);
    }

    fn get_flags(&mut self) -> u8 {
        let mut flags: u8 = 0;
        flags |= (self.carry_flag as u8) << 0;
        flags |= (self.zero_flag as u8) << 1;
        flags |= (self.interrupt_disable_flag as u8) << 2;
        flags |= (self.decimal_mode_flag as u8) << 3;
        flags |= (self.unused_bit4_flag as u8) << 4;
        flags |= (self.unused_bit5_flag as u8) << 5;
        flags |= (self.overflow_flag as u8) << 6;
        flags |= (self.negative_flag as u8) << 7;
        return flags;
    }

    fn set_flags(&mut self, flags: u8) {
        self.carry_flag = flags >> 0 & 1 == 1;
        self.zero_flag = flags >> 1 & 1 == 1;
        self.interrupt_disable_flag = flags >> 2 & 1 == 1;
        self.decimal_mode_flag = flags >> 3 & 1 == 1;
        self.overflow_flag = flags >> 6 & 1 == 1;
        self.negative_flag = flags >> 7 & 1 == 1;
    }

    // Jump to SubRoutine
    fn jsr(&mut self, step_info: StepInfo) {
        let push_address = self.pc - 1;
        self.push16(push_address);

        self.pc = step_info.address;
    }

    // Transfer X Index to Stack Pointer
    fn txs(&mut self, _: StepInfo) {
        self.sp = self.x;
    }

    // Rotate Left
    fn rol(&mut self, step_info: StepInfo) {
        match step_info.addressing_mode {
            AddressingMode::Accumulator => {
                let old_carry = self.carry_flag as u8;
                if self.a & 0x80 == 0x80 {
                    self.carry_flag = true;
                } else {
                    self.carry_flag = false;
                }
                self.a = (self.a << 1) | (old_carry);
                let a = self.a;
                self.set_negative(a);
                self.set_zero(a);
            },
            _ => {
                let old_carry = self.carry_flag as u8;
                let mut data = self.mem.read(step_info.address);
                if data & 0x80 == 0x80 {
                    self.carry_flag = true;
                } else {
                    self.carry_flag = false;
                }
                data = (data << 1) | (old_carry);
                self.mem.write(self.cycles, step_info.address, data);
                self.set_negative(data);
                self.set_zero(data);
            },
        }
    }

    // Rotate Right
    fn ror(&mut self, step_info: StepInfo) {
        match step_info.addressing_mode {
            AddressingMode::Accumulator => {
                let old_carry = self.carry_flag as u8;
                if self.a & 1 == 1 {
                    self.carry_flag = true;
                } else {
                    self.carry_flag = false;
                }
                self.a = (self.a >> 1) | (old_carry << 7);
                let a = self.a;
                self.set_negative(a);
                self.set_zero(a);
            },
            _ => {
                let old_carry = self.carry_flag as u8;
                let mut data = self.mem.read(step_info.address);
                if data & 1 == 1 {
                    self.carry_flag = true;
                } else {
                    self.carry_flag = false;
                }
                data = (data >> 1) | (old_carry << 7);
                self.mem.write(self.cycles, step_info.address, data);
                self.set_negative(data);
                self.set_zero(data);
            },
        }
    }

    // Return from Interrupt
    fn rti(&mut self, _: StepInfo) {
        let flags = self.pop();
        self.set_flags(flags);
        self.pc = self.pop16();
    }

    // RTS - Return from Subroutine
    fn rts(&mut self, _: StepInfo) {
        self.pc = self.pop16() + 1;
    }

    // Set Decimal Flag
    fn sed(&mut self, _: StepInfo) {
        self.decimal_mode_flag = true;
    }

    // Store Accumulator In Memory
    fn sta(&mut self, step_info: StepInfo) {
        self.mem.write(self.cycles, step_info.address, self.a);
    }

    // Store X Index In Memory
    fn stx(&mut self, step_info: StepInfo) {
        self.mem.write(self.cycles, step_info.address, self.x);
    }

    // Store Y Index In Memory
    fn sty(&mut self, step_info: StepInfo) {
        self.mem.write(self.cycles, step_info.address, self.y);
    }

    // Jump to address
    fn jmp(&mut self, step_info: StepInfo) {
        self.pc = step_info.address;
    }

    // For unimplemented instructions
    fn unimplemented(&mut self, step_info: StepInfo) {
        panic!("opcode {:#04X} not implemented", step_info.opcode);
    }
 
    // Push a 16 bit value onto the stack
    fn push16(&mut self, value: u16) {
        let hi = (value >> 8) as u8;
        let lo = (value & 0xFF) as u8;
        self.push(hi);
        self.push(lo);
    }

    // Push a value onto the stack
    fn push(&mut self, value: u8) {
        self.mem.write(self.cycles, 0x0100 | self.sp as u16, value);
        self.sp -= 1;
    }

    // Pop a value from the stack
    fn pop(&mut self) -> u8 {
        self.sp += 1;
        return self.mem.read(0x0100 | self.sp as u16);
    }

    // Push a 16 bit value from the stack
    fn pop16(&mut self) -> u16 {
        let lo = self.pop();
        let hi = self.pop();

        return ((hi as u16) << 8) | lo as u16;
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        return self.mem.read(addr);
    }

    fn read16(&mut self, addr: u16) -> u16 {
        let lo = self.mem.read(addr);
        let hi = self.mem.read(addr + 1);
        return ((hi as u16) << 8) | lo as u16;
    }

    // read 2 bytes from memory. This wraps around when the low byte is 0xFF.
    // For example if addr = 0x0CFF. This will read from 0x0CFF and 0x0C00.
    fn read16_low_byte_wrap(&mut self, addr: u16) -> u16 {
        let addr_low = addr as u16;
        let addr_hi = (addr & 0xFF00) | ((addr as u8).wrapping_add(1)) as u16;
        return ((self.mem.read(addr_hi) as u16) << 8) | (self.mem.read(addr_low) as u16);
    }

    // read 16 bits from zero page address. This includes wrap around for 0xFF
    fn read16_zero_page(&mut self, addr: u8) -> u16 {
        let addr_low = addr as u16;
        let mut addr_hi = 0x0000;
        if addr != 0xFF {
            addr_hi = (addr.wrapping_add(1)) as u16;
        };
        return ((self.mem.read(addr_hi) as u16) << 8) | (self.mem.read(addr_low) as u16);
    }
}

#[derive(Clone)]
struct StepInfo {
    opcode: u8,
    address: u16,
    addressing_mode: AddressingMode,
}

#[derive(Clone)]
enum AddressingMode {
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Accumulator,
    IndexedIndirect,
    Indirect,
    IndirectIndexed,
    Immediate,
    Implied,
    Relative,
    Unknown,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
}

pub struct CPUMemory {
    pub mapper: Rc<RefCell<Box<dyn mapper::Mapper>>>,
    pub ram: [u8; 2048],
    pub ppu: Rc<RefCell<ppu::PPU<ppu::PPUMemory>>>,
    pub apu: Rc<RefCell<apu::APU>>,
    pub controller1: Rc<RefCell<controller::Controller>>,
    pub added_stall: u32,
}

impl Memory for CPUMemory {
    fn read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            let ram_index = addr % 0x0800;
            return self.ram[ram_index as usize];
        } else if addr >= 0x2000 && addr < 0x4000 {
            return self.ppu.borrow_mut().read_register(addr);
        } else if addr >= 0x4000 && addr < 0x4014 {
            // TODO
        } else if addr == 0x4015 {
            // TODO
        } else if addr == 0x4016 {
            return self.controller1.borrow_mut().read_next_button_state();
        } else if addr == 0x4017 {
            // TODO
        } else if addr < 0x4018 {
            // TODO implement this
            //unimplemented!();
        } else if addr >= 0x6000 {
            return self.mapper.borrow_mut().read(addr);
        } 
        return 0;
    }

    fn write(&mut self, cycle: u64, addr: u16, data: u8) {
        if addr < 0x2000 {
            let ram_index = addr % 0x0800;
            self.ram[ram_index as usize] = data;
        } else if addr >= 0x2000 && addr < 0x4000 {
            self.ppu.borrow_mut().write_register(addr, data);
        } else if addr >= 0x4000 && addr < 0x4014 {
            self.apu.borrow_mut().write_register(addr, data);
        } else if addr == 0x4014 {
            let mut address = (data as u16) << 8;
            let mut oam_data = [0; 256];
            for i in 0..256 {
                oam_data[i] = self.read(address);
                address += 1;
            }
            self.ppu.borrow_mut().wrote_to_register(data);
            self.ppu.borrow_mut().write_oamdma(oam_data);
            self.added_stall += 513;
            if cycle%2 == 1 {
                self.added_stall += 1;
            }
        } else if addr == 0x4015 {
            self.apu.borrow_mut().write_register(addr, data);
        } else if addr == 0x4016 {
            if data&1 == 1 {
                self.controller1.borrow_mut().set_strobe(true);
            } else {
                self.controller1.borrow_mut().set_strobe(false);
            }
        } else if addr == 0x4017 {
            // TODO joy pad 2
            self.apu.borrow_mut().write_register(addr, data);
        } else if addr < 0x4018 {
            // TODO implement this
            unimplemented!();
        } else if addr >= 0x6000 {
            self.mapper.borrow_mut().write(addr, data);
        }
    }

    fn get_added_stall(&mut self) -> u32 {
        let added_stall = self.added_stall;
        self.added_stall = 0;
        return added_stall;
    }
}