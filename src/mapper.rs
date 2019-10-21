
use cpu;
use ppu;

use std::rc::Rc;
use std::cell::RefCell;

pub trait Mapper {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, data: u8); 
    fn get_chr(&mut self) -> Vec<u8>;
    fn step(&mut self, ppu: &Rc<RefCell<ppu::PPU<ppu::PPUMemory>>>, cpu: &mut cpu::CPU<cpu::CPUMemory>);
}

struct Mapper0 {
    chr: Vec<u8>,
    prg: Vec<u8>,
    save_ram: [u8; 8192],
}

impl Mapper0 {
    fn new(chr: Vec<u8>, prg: Vec<u8>) -> Mapper0 {
        Mapper0{
            chr: chr,
            prg: prg,
            save_ram: [0; 8192],
        }
    }

    fn get_actual_addr(&mut self, addr: u16) -> u16 {
        let num_banks = self.prg.len() / 0x4000;
        let mut address = addr;
        if address >= 0x8000 && address < 0xC000  {
            return address - 0x8000;
        } else if address >= 0xC000 {
            address -= 0xC000;
            if num_banks == 2 {
                return 0x4000 + address;
            } else if num_banks == 1 {
                return address;
            } else {
                unimplemented!();
            }
        } else {
            unimplemented!();
        }
    }
}

impl Mapper for Mapper0 {
    fn read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            return self.chr[addr as usize];
        } else if addr >= 0x6000 && addr < 0x8000 {
            return self.save_ram[(addr-0x6000) as usize];
        } else if addr >= 0x8000 {
            let address = self.get_actual_addr(addr);
            return self.prg[address as usize];
        } else {
            unimplemented!();
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        if addr < 0x2000 {
            self.chr[addr as usize] = data;
        } else if addr >= 0x6000 && addr < 0x8000 {
            self.save_ram[(addr-0x6000) as usize] = data;
        } else {
            unimplemented!();
        }
    }

    fn get_chr(&mut self) -> Vec<u8> {
        return self.chr.clone();
    }

    fn step(&mut self, _ppu: &Rc<RefCell<ppu::PPU<ppu::PPUMemory>>>, _cpu: &mut cpu::CPU<cpu::CPUMemory>) {}
}

struct Mapper1 {
    chr: Vec<u8>,
    prg: Vec<u8>,
    save_ram: [u8; 8192],
    shift_register: u8,
    control: u8,
    prg_bank_mode: u8,
    chr_bank_mode: u8,
    prg_bank: u8,
    chr_0_bank: u8,
    chr_1_bank: u8,
    beginning_fix_last_bank: bool,
    nametable_mirror_type: Rc<RefCell<Box<ppu::NametableMirroring>>>,
}

impl Mapper1 {
    fn new(chr: Vec<u8>, prg: Vec<u8>, nametable_mirror_type: Rc<RefCell<Box<ppu::NametableMirroring>>>) -> Mapper1 {
        Mapper1{
            chr: chr,
            prg: prg,
            save_ram: [0; 8192],
            shift_register: 0x10,
            control: 0,
            prg_bank_mode: 0,
            chr_bank_mode: 0,
            prg_bank: 0,
            chr_0_bank: 0,
            chr_1_bank: 0,
            beginning_fix_last_bank: true,
            nametable_mirror_type: nametable_mirror_type,
        }
    }

    fn write_control(&mut self, v: u8) {
        self.control = v;
        self.prg_bank_mode = (v >> 2) & 3;
        self.chr_bank_mode = (v >> 4) & 1;
        let mirror = v & 3;
        match mirror {
            0 => {
                self.nametable_mirror_type.borrow_mut().update_nametable_mirror_type(ppu::NametableMirrorType::Single0);
            },
            1 => {
                self.nametable_mirror_type.borrow_mut().update_nametable_mirror_type(ppu::NametableMirrorType::Single1);
            }
            2 => {
                self.nametable_mirror_type.borrow_mut().update_nametable_mirror_type(ppu::NametableMirrorType::Vertical);
            },
            3 => {
                self.nametable_mirror_type.borrow_mut().update_nametable_mirror_type(ppu::NametableMirrorType::Horizontal);
            }
            _ => {
                unimplemented!();
            }
        }
    }

    fn write_to_shift_register(&mut self, addr: u16, v: u8) {
        if v&0x80 == 0x80 {
            self.shift_register = 0x10;
            self.write_control(v | 0x0C);
        } else {
            let mut write_finished = false;
            if self.shift_register&0x01 == 0x01 {
                write_finished = true;
            }
            self.shift_register >>= 1;
            self.shift_register |= (v & 1) << 4;
            if write_finished {
                if addr <= 0x9FFF {
                    let sr = self.shift_register.clone();
                    self.write_control(sr);
                } else if addr <= 0xBFFF {
                    self.chr_0_bank = self.shift_register;
                } else if addr <= 0xDFFF {
                    self.chr_1_bank = self.shift_register;
                } else {
                    self.prg_bank = self.shift_register & 0x0F;
                }
                self.beginning_fix_last_bank = false;
                self.shift_register = 0x10;
            } 
        }
    }

    fn get_prg_addr(&mut self, addr: u16) -> usize {
        let addr2 = addr - 0x8000;
        let bank = addr2 / 0x4000; 
        let offset = addr2 % 0x4000;
        if bank == 1 && self.beginning_fix_last_bank {
            let begin_bank_offset = ((self.prg.len() / 0x4000) - 1) * 0x4000;
            return (begin_bank_offset+offset as usize) as usize;
        }
        if self.prg_bank_mode == 0 || self.prg_bank_mode == 1 {
            if bank == 0 {
                let mut index = (self.prg_bank & 0xFE) as usize;
                index %= (self.prg.len() as usize) / 0x4000;
                let begin_bank_offset = index *0x4000;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 1 {
                let mut index = (self.prg_bank | 0x01) as usize;
                index %= (self.prg.len() as usize) / 0x4000;
                let begin_bank_offset = index *0x4000;
                return (begin_bank_offset+offset as usize) as usize;
            } else {
                unimplemented!();
            }
        } else if self.prg_bank_mode == 2 {
            if bank == 0 {
                return offset as usize;
            } else if bank == 1 {
                let mut index = self.prg_bank as u16;
                index %= (self.prg.len() as u16) / 0x4000;
                let begin_bank_offset = index *0x4000;
                return (begin_bank_offset+offset) as usize;
            } else {
                unimplemented!();
            }
        } else if self.prg_bank_mode == 3 {
            if bank == 0 {
                let mut index = self.prg_bank as usize;
                index %= (self.prg.len() as usize) / 0x4000;
                let begin_bank_offset = index * 0x4000;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 1 {
                let begin_bank_offset = ((self.prg.len() / 0x4000) - 1) * 0x4000;
                return (begin_bank_offset+offset as usize) as usize;
            } else {
                unimplemented!();
            }
        } 
        unimplemented!();
    }

    fn get_chr_addr(&mut self, addr: u16) -> usize {
        let bank = addr / 0x1000; 
        let offset = addr % 0x1000;
        if self.chr_bank_mode == 0 {
            if bank == 0 {
                let mut index = (self.chr_0_bank & 0xFE) as usize;
                index %= (self.prg.len() as usize) / 0x1000;
                let begin_bank_offset = index * 0x1000;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 1 {
                let mut index = (self.chr_0_bank | 0x01) as usize;
                index %= (self.prg.len() as usize) / 0x1000;
                let begin_bank_offset = index * 0x1000;
                return (begin_bank_offset+offset as usize) as usize;
            } else {
                unimplemented!();
            }
        } else if self.chr_bank_mode == 1{
            if bank == 0 {
                let mut index = self.chr_0_bank as u16;
                index %= (self.prg.len() as u16) / 0x1000;
                let begin_bank_offset = index * 0x1000;
                return (begin_bank_offset+offset) as usize;
            } else if bank == 1 {
                let mut index = self.chr_1_bank as u16;
                index %= (self.prg.len() as u16) / 0x1000;
                let begin_bank_offset = index * 0x1000;
                return (begin_bank_offset+offset) as usize;
            } else {
                unimplemented!();
            }
        }
        unimplemented!();
    }
}

impl Mapper for Mapper1 {
    fn read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            let chr_addr = self.get_chr_addr(addr);
            return self.chr[chr_addr];
        } else if addr >= 0x6000 && addr < 0x8000 {
            return self.save_ram[(addr-0x6000) as usize];
        } else if addr >= 0x8000 {
            let prg_addr = self.get_prg_addr(addr);
            return self.prg[prg_addr];
        } else {
            unimplemented!();
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        if addr < 0x2000 {
            let chr_addr = self.get_chr_addr(addr);
            self.chr[chr_addr] = data;
        } else if addr >= 0x6000 && addr < 0x8000 {
            self.save_ram[(addr-0x6000) as usize];
        } else if addr >= 0x8000 {
            self.write_to_shift_register(addr, data);
        } else {
            unimplemented!();
        }
    }

    fn get_chr(&mut self) -> Vec<u8> {
        return self.chr.clone();
    }

    fn step(&mut self, _ppu: &Rc<RefCell<ppu::PPU<ppu::PPUMemory>>>, _cpu: &mut cpu::CPU<cpu::CPUMemory>) {}
}

struct Mapper2 {
    chr: Vec<u8>,
    prg: Vec<u8>,
    save_ram: [u8; 8192],
    selected_bank_1: u8,
}

impl Mapper2 {
    fn new(chr: Vec<u8>, prg: Vec<u8>) -> Mapper2 {
        Mapper2{
            chr: chr,
            prg: prg,
            save_ram: [0; 8192],
            selected_bank_1: 0,
        }
    }

    fn num_banks(&mut self) -> usize {
        return self.prg.len() / 0x4000;
    }
}

impl Mapper for Mapper2 {
    fn read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            return self.chr[addr as usize];
        } else if addr >= 0x6000 && addr < 0x8000 {
            return self.save_ram[(addr-0x6000) as usize];
        } else if addr >= 0x8000 && addr < 0xC000 {
            let address = (self.selected_bank_1 as u32)*(0x4000 as u32) + ((addr - 0x8000) as u32);
            return self.prg[address as usize];
        } else if addr >= 0xC000 {
            let last_bank = (self.prg.len() / 0x4000) - 1;
            let address = (last_bank as u32)*(0x4000 as u32) + ((addr - 0xC000) as u32);
            return self.prg[address as usize];
        } else {
            unimplemented!();
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        if addr < 0x2000 {
            self.chr[addr as usize] = data;
        } else if addr >= 0x6000 && addr < 0x8000 {
            self.save_ram[(addr-0x6000) as usize] = data;
        } else if addr >= 0x8000 {
            self.selected_bank_1 = ((data as u16) % (self.num_banks() as u16)) as u8;
        } else {
            unimplemented!();
        }
    }

    fn get_chr(&mut self) -> Vec<u8> {
        return self.chr.clone();
    }

    fn step(&mut self, _ppu: &Rc<RefCell<ppu::PPU<ppu::PPUMemory>>>, _cpu: &mut cpu::CPU<cpu::CPUMemory>) {}
}

struct Mapper3 {
    chr: Vec<u8>,
    prg: Vec<u8>,
    save_ram: [u8; 8192],
    selected_chr_bank: u8,
}


impl Mapper3 {
    fn new(chr: Vec<u8>, prg: Vec<u8>) -> Mapper3 {
        Mapper3{
            chr: chr,
            prg: prg,
            save_ram: [0; 8192],
            selected_chr_bank: 0,
        }
    }

    fn get_actual_addr(&mut self, addr: u16) -> u16 {
        let num_banks = self.prg.len() / 0x4000;
        let mut address = addr;
        if address >= 0x8000 && address < 0xC000  {
            return address - 0x8000;
        } else if address >= 0xC000 {
            address -= 0xC000;
            if num_banks == 2 {
                return 0x4000 + address;
            } else if num_banks == 1 {
                return address;
            } else {
                unimplemented!();
            }
        } else {
            unimplemented!();
        }
    }
}

impl Mapper for Mapper3 {
    fn read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            let address = (self.selected_chr_bank as u32)*(0x2000 as u32) + (addr as u32);
            return self.chr[address as usize];
        } else if addr >= 0x6000 && addr < 0x8000 {
            return self.save_ram[(addr-0x6000) as usize];
        } else if addr >= 0x8000 {
            let address = self.get_actual_addr(addr);
            return self.prg[address as usize];
        } else {
            unimplemented!();
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        if addr < 0x2000 {
            let address = (self.selected_chr_bank as u32)*(0x2000 as u32) + (addr as u32);
            self.chr[address as usize] = data;
        } else if addr >= 0x6000 && addr < 0x8000 {
            self.save_ram[(addr-0x6000) as usize] = data;
        } else if addr >= 0x8000 {
            self.selected_chr_bank = data;
        } else {
            unimplemented!();
        }
    }

    fn get_chr(&mut self) -> Vec<u8> {
        return self.chr.clone();
    }

    fn step(&mut self, _ppu: &Rc<RefCell<ppu::PPU<ppu::PPUMemory>>>, _cpu: &mut cpu::CPU<cpu::CPUMemory>) {}
}

struct Mapper4 {
    chr: Vec<u8>,
    prg: Vec<u8>,
    save_ram: [u8; 8192],
    nametable_mirror_type: Rc<RefCell<Box<ppu::NametableMirroring>>>,
    bank_registers: [u8; 8],
    selected_bank_register: u8,
    prg_bank_mode: u8,
    chr_bank_mode: u8,
    irq_enable: bool,
    irq_counter: u8,
    irq_counter_reload_value: u8,
    startup_banks: bool,

}

impl Mapper4 {
    fn new(chr: Vec<u8>, prg: Vec<u8>, nametable_mirror_type: Rc<RefCell<Box<ppu::NametableMirroring>>>) -> Mapper4 {
        return Mapper4{
            chr: chr,
            prg: prg,
            save_ram: [0; 8192],
            nametable_mirror_type: nametable_mirror_type,
            bank_registers: [0; 8],
            selected_bank_register: 0,
            prg_bank_mode: 0,
            chr_bank_mode: 0,
            irq_enable: false,
            irq_counter: 0,
            irq_counter_reload_value: 0,
            startup_banks: true,
        }
    }

    fn write_register(&mut self, addr: u16, v: u8) {
        if addr >= 0x8000 && addr <= 0x9FFF && addr%2 == 0 {
            self.selected_bank_register = v&7;
            self.prg_bank_mode = (v >> 6) & 1;
            self.chr_bank_mode = (v >> 7 ) & 1;
            self.startup_banks = false;
        } else if addr >= 0x8000 && addr <= 0x9FFF && addr%2 == 1 {
            self.bank_registers[self.selected_bank_register as usize] = v;
            self.startup_banks = false;
        } else if addr >= 0xA000 && addr <= 0xBFFF && addr%2== 0 {
            self.write_mirror(v);
        } else if addr >= 0xA000 && addr <= 0xBFFF && addr%2 == 1 {
            // TODO prg ram protect
        } else if addr >= 0xC000 && addr <= 0xDFFF && addr%2 == 0 {
            self.irq_counter_reload_value = v;
        } else if addr >= 0xC000 && addr <= 0xDFFF && addr %2 == 1 {
            self.irq_counter = 0; // TODO is this really correct ?
        } else if addr >= 0xE000 && addr %2 == 0 {
            self.irq_enable = false;
        } else if addr >= 0xE000 && addr %2 == 1 {
            self.irq_enable = true;
        } else {
            unimplemented!();
        }
    }

    fn get_prg_addr(&mut self, addr: u16) -> usize {
        let addr2 = addr - 0x8000;
        let bank = addr2 / 0x2000; 
        let offset = addr2 % 0x2000;
        if self.startup_banks {
            if bank == 0 {
                return offset as usize;
            } else if bank == 1 {
                return (0x2000+offset as usize) as usize;
            } else if bank == 2 {
                let begin_bank_offset = ((self.prg.len() / 0x2000) - 2) * 0x2000;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 3 {
                let begin_bank_offset = ((self.prg.len() / 0x2000) - 1) * 0x2000;
                return (begin_bank_offset+offset as usize) as usize;
            } else {
                unimplemented!();
            }
        } else if self.prg_bank_mode == 0 {
            if bank == 0 {
                let mut index = self.bank_registers[6] as usize;
                index %= (self.prg.len() as usize) / 0x2000;
                let begin_bank_offset = index *0x2000;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 1 {
                let mut index = self.bank_registers[7] as usize;
                index %= (self.prg.len() as usize) / 0x2000;
                let begin_bank_offset = index *0x2000;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 2 {
                let begin_bank_offset = ((self.prg.len() / 0x2000) - 2) * 0x2000;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 3 {
                let begin_bank_offset = ((self.prg.len() / 0x2000) - 1) * 0x2000;
                return (begin_bank_offset+offset as usize) as usize;
            } else {
                unimplemented!();
            }
        } else if self.prg_bank_mode == 1 {
            if bank == 0 {
                let begin_bank_offset = ((self.prg.len() / 0x2000) - 2) * 0x2000;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 1 {
                let mut index = self.bank_registers[7] as usize;
                index %= (self.prg.len() as usize) / 0x2000;
                let begin_bank_offset = index *0x2000;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 2 {
                let mut index = self.bank_registers[6] as usize;
                index %= (self.prg.len() as usize) / 0x2000;
                let begin_bank_offset = index *0x2000;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 3 {
                let begin_bank_offset = ((self.prg.len() / 0x2000) - 1) * 0x2000;
                return (begin_bank_offset+offset as usize) as usize;
            } else {
                unimplemented!();
            }
        } else {
            unimplemented!();
        }
    }

    fn get_chr_addr(&mut self, addr: u16) -> usize {
        let bank = addr / 0x400; 
        let offset = addr % 0x400;

        if self.chr_bank_mode == 0 {
            if bank == 0 {
                let mut index = (self.bank_registers[0] & 0xFE) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 1 {
                let mut index = (self.bank_registers[0] | 0x01) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 2 {
                let mut index = (self.bank_registers[1] & 0xFE) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 3 {
                let mut index = (self.bank_registers[1] | 0x01) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 4 {
                let mut index = (self.bank_registers[2]) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 5 {
                let mut index = (self.bank_registers[3]) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 6 {
                let mut index = (self.bank_registers[4]) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 7 {
                let mut index = (self.bank_registers[5]) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else {
                unimplemented!();
            }
        } else if self.chr_bank_mode == 1 {
            if bank == 0 {
                let mut index = (self.bank_registers[2]) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 1 {
                let mut index = (self.bank_registers[3]) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 2 {
                let mut index = (self.bank_registers[4]) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 3 {
                let mut index = (self.bank_registers[5]) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 4 {
                let mut index = (self.bank_registers[0] & 0xFE) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 5 {
                let mut index = (self.bank_registers[0] | 0x01) as usize;
                index %= (self.prg.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 6 {
                let mut index = (self.bank_registers[1] & 0xFE) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else if bank == 7 {
                let mut index = (self.bank_registers[1] | 0x01) as usize;
                index %= (self.chr.len() as usize) / 0x400;
                let begin_bank_offset = index * 0x400;
                return (begin_bank_offset+offset as usize) as usize;
            } else {
                unimplemented!();
            }
        } else {
            unimplemented!();
        }
    }

    fn write_mirror(&mut self, v: u8) {
        match v&1 {
            0 => {
                self.nametable_mirror_type.borrow_mut().update_nametable_mirror_type(ppu::NametableMirrorType::Vertical);
            },
            1 => {
                self.nametable_mirror_type.borrow_mut().update_nametable_mirror_type(ppu::NametableMirrorType::Horizontal);
            },
            _ => {
                unimplemented!();
            }
        }
    }
}

impl Mapper for Mapper4 {
    fn read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            let chr_addr = self.get_chr_addr(addr);
            return self.chr[chr_addr];
        } else if addr >= 0x6000 && addr < 0x8000 {
            return self.save_ram[(addr-0x6000) as usize];
        } else if addr >= 0x8000 {
            let prg_addr = self.get_prg_addr(addr);
            return self.prg[prg_addr];
        } else {
            unimplemented!();
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        if addr < 0x2000 {
            let chr_addr = self.get_chr_addr(addr);
            self.chr[chr_addr] = data;
        } else if addr >= 0x6000 && addr < 0x8000 {
            self.save_ram[(addr-0x6000) as usize] = data;
        } else if addr >= 0x8000 {
            self.write_register(addr, data);
        } else {
            unimplemented!();
        }
    }

    fn get_chr(&mut self) -> Vec<u8> {
        return self.chr.clone();
    }

    fn step(&mut self, ppu: &Rc<RefCell<ppu::PPU<ppu::PPUMemory>>>, cpu: &mut cpu::CPU<cpu::CPUMemory>) {
        if ppu.borrow().cycle() != 260 {
            return;
        }
        if ppu.borrow().scanline() > 239 && ppu.borrow().scanline() < 261 {
            return;
        }
        if !ppu.borrow().get_show_background_flag() && !ppu.borrow().get_show_sprite_flag() {
            return;
        }
        if self.irq_counter == 0 {
            self.irq_counter = self.irq_counter_reload_value;
        } else {
            self.irq_counter -= 1;
            if self.irq_counter == 0 && self.irq_enable {
                cpu.set_irq();
            }
        }
    }
}

struct Mapper7 {
    chr: Vec<u8>,
    prg: Vec<u8>,
    save_ram: [u8; 8192],
    nametable_mirror_type: Rc<RefCell<Box<ppu::NametableMirroring>>>,
    selected_bank: u8,
}

impl Mapper7 {
    fn new(chr: Vec<u8>, prg: Vec<u8>, nametable_mirror_type: Rc<RefCell<Box<ppu::NametableMirroring>>>) -> Mapper7 {
        return Mapper7{
            chr: chr,
            prg: prg,
            save_ram: [0; 8192],
            selected_bank: 0,
            nametable_mirror_type: nametable_mirror_type,
        }
    }
}

impl Mapper for Mapper7 {
    fn read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            return self.chr[addr as usize];
        } else if addr >= 0x6000 && addr < 0x8000 {
            return self.save_ram[(addr-0x6000) as usize];
        } else if addr >= 0x8000 {
            let address = ((self.selected_bank as u32)*0x8000 as u32) + (addr-0x8000) as u32;
            return self.prg[address as usize];
        } else {
            unimplemented!();
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        if addr < 0x2000 {
            self.chr[addr as usize] = data;
        } else if addr >= 0x6000 && addr < 0x8000 {
            self.save_ram[(addr-0x6000) as usize] = data;
        } else if addr >= 0x8000 {
            self.selected_bank = data & 7;
            match (data >> 4) & 1 {
                0 => {
                    self.nametable_mirror_type.borrow_mut().update_nametable_mirror_type(ppu::NametableMirrorType::Single0);
                },
                1 => {
                    self.nametable_mirror_type.borrow_mut().update_nametable_mirror_type(ppu::NametableMirrorType::Single1);
                },
                _ => unimplemented!(),
            }
        } else {
            unimplemented!();
        }
    }

    fn get_chr(&mut self) -> Vec<u8> {
        return self.chr.clone();
    }

    fn step(&mut self, _ppu: &Rc<RefCell<ppu::PPU<ppu::PPUMemory>>>, _cpu: &mut cpu::CPU<cpu::CPUMemory>) {}
}


pub fn create_mapper(mapper: u8, chr: Vec<u8>, prg: Vec<u8>, nametable_mirror_type: Rc<RefCell<Box<ppu::NametableMirroring>>>) -> Box<dyn Mapper> {
    match mapper{
        0 => {
            return Box::new(Mapper0::new(chr, prg));
        },
        1 => {
            return Box::new(Mapper1::new(chr, prg, nametable_mirror_type));
        },
        2 => {
            return Box::new(Mapper2::new(chr, prg));
        },
        3 => {
            return Box::new(Mapper3::new(chr, prg));
        },
        4 => {
            return Box::new(Mapper4::new(chr, prg, nametable_mirror_type));
        }
        7 => {
            return Box::new(Mapper7::new(chr, prg, nametable_mirror_type));
        },
        _ => {
            panic!("Game uses unsupported mapper {:}", mapper);
        }
    }
}