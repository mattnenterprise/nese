use std::fs::File;
use std::io::prelude::*;
use std::str;

const INES_HEADER_MAGIC_NUMBER: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
pub const PRG_ROM_UNIT_SIZE: u32 = 16384;
const CHR_ROM_UNIT_SIZE: u32 = 8192;

pub struct INESData {
    pub prg: Vec<u8>,
    pub chr: Vec<u8>,
    pub mapper: u8,
    pub nametable_mirroring: u8,
}

// https://wiki.nesdev.com/w/index.php/INES
pub fn load_ines_file(file_name: &str) -> INESData {
    // TODO need to acknowledge error instead of unwrap
    let mut file = File::open(file_name).unwrap();

    let mut header_magic_number: [u8; 4] = [0; 4];
    // TODO need to acknowledge error instead of unwrap
    file.read_exact(&mut header_magic_number).unwrap();

    if header_magic_number != INES_HEADER_MAGIC_NUMBER {
        // TODO need to acknowledge error if the  magic number if incorrect 
        panic!("Error with header magic number")
    }

    let mut prg_rom_size: [u8; 1] = [0];
    // TODO need to acknowledge error instead of unwrap
    file.read_exact(&mut prg_rom_size).unwrap();

    let mut chr_rom_size: [u8; 1] = [0];
    // TODO need to acknowledge error instead of unwrap
    file.read_exact(&mut chr_rom_size).unwrap();

    let mut flags6: [u8; 1] = [0];
    // TODO need to acknowledge error instead of unwrap
    file.read_exact(&mut flags6).unwrap();

    let mut flags7: [u8; 1] = [0];
    // TODO need to acknowledge error instead of unwrap
    file.read_exact(&mut flags7).unwrap();
    if flags7[0] & 0x0C == 0x08 {
        // TODO then nes 2.0
    } else if flags7[0] & 0x0C == 0 {
        // TODO then iNES
    } else {
        // archaic iNES
        flags7[0] = 0;
    }

    let mut mapper = flags6[0]>>4;
    mapper = mapper | flags7[0]&0xF0;

    let low_mirror = flags6[0] & 1;
    let high_mirror = (flags6[0] >> 3) & 1;
    let mut nametable_mirroring = low_mirror;
    if high_mirror != 0 {
        nametable_mirroring = 4;
    }

    if mapper != 0 && mapper != 2 && mapper != 3 && mapper != 7 && mapper != 1 && mapper != 4 {
        // TODO properly propagate this error up
        panic!("mapper is {}, but we can only emulate mapper 0, 2, 3, 7, 1, and 4 at this time.", mapper);
    }

    // TODO ignoring the last 8 bytes, but they may be useful at some point
    let mut ignore: [u8; 8] = [0; 8];
    // TODO need to acknowledge error instead of unwrap
    file.read_exact(&mut ignore).unwrap();

    // Read the trainer if it is present.
    // TODO this will currently ignore the trainer after reading it. I don't think it is crucial to the operation of most ROMs.
    if flags6[0] & 0x04 == 4 {
        let mut trainer: [u8; 512] = [0; 512];
        // TODO need to acknowledge error instead of unwrap
        file.read_exact(&mut trainer).unwrap();
    }


    let mut prg_rom = vec![0u8; prg_rom_size[0] as usize * PRG_ROM_UNIT_SIZE as usize];
    // TODO need to acknowledge error instead of unwrap
    file.read_exact(&mut prg_rom).unwrap();

    let mut chr_rom = vec![0u8; chr_rom_size[0] as usize * CHR_ROM_UNIT_SIZE as usize];
    // TODO need to acknowledge error instead of unwrap
    file.read_exact(&mut chr_rom).unwrap();

    if chr_rom_size[0] == 0 {
        chr_rom = vec![0u8; CHR_ROM_UNIT_SIZE as usize];
    }

    INESData{
        prg: prg_rom,
        chr: chr_rom,
        mapper: mapper,
        nametable_mirroring: nametable_mirroring,
    }
}
