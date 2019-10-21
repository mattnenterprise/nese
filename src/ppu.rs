use image::{ImageBuffer,Rgb};
use mapper;
use std::rc::Rc;
use std::cell::RefCell;

const PALETTE: [[u8; 3]; 64] = [ 
    [84, 84, 84],
    [0, 30, 116],
    [8, 16, 144],
    [48, 0, 136],
    [68, 0, 100],
    [92, 0, 48],
    [84, 4, 0],
    [60, 24, 0],
    [32, 42, 0],
    [8, 58, 0],
    [0, 64, 0],
    [0, 60, 0],
    [0, 50, 60],
    [0, 0, 0],
    [0,0,0],
    [0,0,0],
    [152, 150, 152],
    [8, 76, 196],
    [48, 50, 236],
    [92, 30, 228],
    [136, 20, 176],
    [160, 20, 100],
    [152, 34, 32],
    [120, 60, 0],
    [84, 90, 0],
    [40, 114, 0],
    [8, 124, 0],
    [0, 118, 40],
    [0, 102, 120],
    [0, 0, 0],
    [0,0,0],
    [0,0,0],
    [236, 238, 236],
    [76, 154, 236],
    [120, 124, 236],
    [176,  98, 236],
    [228,  84, 236],
    [236,  88, 180],
    [236, 106, 100],
    [212, 136, 32],
    [160, 170, 0],
    [116, 196, 0],
    [76, 208, 32],
    [56, 204, 108],
    [56, 180, 204],
    [60, 60, 60],
    [0,0,0],
    [0,0,0],
    [236, 238, 236],
    [168, 204, 236],
    [188, 188, 236],
    [212, 178, 236],
    [236, 174, 236],
    [236, 174, 212],
    [236, 180, 176],
    [228, 196, 144],
    [204, 210, 120],
    [180, 222, 120],
    [168, 226, 144],
    [152, 226, 180],
    [160, 214, 228],
    [160, 162, 160],
    [0,0,0],
    [0,0,0],
];

#[derive(Clone, Debug)]
pub enum NametableMirrorType {
    Horizontal,
    Vertical,
    Single0,
    Single1,
    Four,
}

pub struct NametableMirroring {
   pub nametable_mirror_type: NametableMirrorType,
}

impl NametableMirroring {
    pub fn update_nametable_mirror_type(&mut self, mirror_type: NametableMirrorType) {
        self.nametable_mirror_type = mirror_type;
    }

    pub fn get_nametable_mirror_type(&mut self) -> NametableMirrorType {
        return self.nametable_mirror_type.clone();
    }
}

pub trait Memory {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, data: u8);
    fn get_nametable_index(&mut self, address: u16) -> u16;
}

pub struct PPU<T: Memory> {
    mem: T,

    nmi_occurred: bool,
    nmi_output: bool,

    // $2000: PPUCTRL
    base_nametable_addr_flag: u8, // 0: $2000; 1: $2400; 2: $2800; 3: $2C00
    vram_increment_flag: u8, // 0: add 1; 1: add 32
    sprite_table_addr_flag: u8, // 0: $0000; 1: $1000; ignored in 8x16 mode
    background_table_addr_flag: u8, // 0: $0000; 1: $1000
    sprite_size_flag: u8, // 0: 8x8; 1: 8x16
    master_slave_flag: u8, // 0: read backdrop from EXT pins; 1: output color on EXT pins

    // $2001: PPUMASK
    greyscale_flag: u8, // 0: normal color, 1: produce a greyscale display
    show_left_background_flag: u8, // 1: Show background in leftmost 8 pixels of screen, 0: Hide
    show_left_sprites_flag: u8, // 1: Show sprites in leftmost 8 pixels of screen, 0: Hide
    show_background_flag: u8, // 1: Show background
    show_sprites_flag: u8, // 1: Show sprites
    emphasize_red_flag: u8, 
    emphasize_green_flag: u8,
    emphasize_blue_flag: u8,

    // $2002: PPUSTATUS
    sprite_overflow_flag: bool,
    sprite_zero_hit_flag: bool,

    v: u16, // Current VRAM address (15 bits)
    t: u16, // Temporary VRAM address (15 bits); can also be thought of as the address of the top left onscreen tile.
    x: u8, // Fine X scroll (3 bits)
    w: u8, // First or second write toggle (1 bit)

    // The previous data written to a PPU register.
    previous_write_data: u8,

    nametable_data: [u8; 2048],

    oam_addr: u8,
    oam_data: [u8; 256],

    ppu_data_buffer: u8,

    palette_data: [u8; 32],

    nametable_byte: u8,
    attribute_byte: u8,
    low_bg_tile_byte: u8,
    high_bg_tile_byte: u8,

    low_bit_bitmap_bg_shift_register: u16,
    high_bit_bitmap_bg_shift_register: u16,
    low_bit_palette_attr_bg_shift_register: u16,
    high_bit_palette_attr_bg_shift_register: u16,

    sprite_attributes: [u8; 8],
    sprite_positions: [u8; 8],
    sprite_indexes: [u8; 8],
    sprite_count: u8,
    low_bit_sprite_bitmaps: [u8; 8],
    high_bit_sprite_bitmaps: [u8; 8],


    scanline: u16, // 0-261: Pre-render 261 , Visible 0-239 , Post-render 240 , Vertical Blanking 241-260
    cycle: u16, // 0-340

    pub frame_buffer: ImageBuffer<Rgb<u8>, Vec<u8>>,
}

pub struct StepOutput {
    pub nmi: bool,
    pub frame_change: bool,
}

impl<T: Memory> PPU<T> {
    pub fn new(mem: T) -> PPU<T> {
        PPU{
            mem: mem,

            nmi_occurred: false,
            nmi_output: false,

            base_nametable_addr_flag: 0,
            vram_increment_flag: 0,
            sprite_table_addr_flag: 0,
            background_table_addr_flag: 0,
            sprite_size_flag: 0,
            master_slave_flag: 0,

            greyscale_flag: 0,
            show_left_background_flag: 0,
            show_left_sprites_flag: 0,
            show_background_flag: 0,
            show_sprites_flag: 0,
            emphasize_red_flag: 0,
            emphasize_green_flag: 0,
            emphasize_blue_flag: 0,


            sprite_overflow_flag: false,
            sprite_zero_hit_flag: false,

            v: 0,
            t: 0,
            x: 0,
            w: 0,

            previous_write_data: 0,

            nametable_data: [0; 2048],

            oam_addr: 0,
            oam_data: [0; 256],

            ppu_data_buffer: 0,

            palette_data: [0; 32],

            nametable_byte: 0,
            attribute_byte:0,
            low_bg_tile_byte: 0,
            high_bg_tile_byte: 0,

            low_bit_bitmap_bg_shift_register: 0,
            high_bit_bitmap_bg_shift_register: 0,
            low_bit_palette_attr_bg_shift_register: 0,
            high_bit_palette_attr_bg_shift_register: 0,

            sprite_attributes: [0; 8],
            sprite_positions: [0; 8],
            sprite_indexes: [0; 8],
            sprite_count: 0,
            low_bit_sprite_bitmaps: [0; 8],
            high_bit_sprite_bitmaps: [0; 8],

            scanline: 241,
            cycle: 0,

            frame_buffer: ImageBuffer::new(256, 240),
        }
    }

    pub fn read_register(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 || addr >= 0x4000 {
            panic!("read register addr {:} is out of range", addr);
        }
        match addr % 8 {
           1 => {
               // TODO is this really what this should do ?
               // Ice Climbers tries to read from address 0x2BA9 which == 0x2001 after mirroring.
               return 0;
           },
           2 => {
               let status = self.read_status();
               return status;
           },
           3 => {
               // TODO is this suppose to be here ?
               return 0;
           }
           4 => {
               return self.read_oam_data();
           },
           5 => {
               // TODO is this suppose to be here
               return 0;
           },
           7 => {
               let data = self.read_data();
               return data;
           }
           _ => panic!("Problem reading PPU register {:#04X}", addr),
        };
    }

    pub fn wrote_to_register(&mut self, data: u8) {
        self.previous_write_data = data;
    }

    pub fn write_register(&mut self, addr: u16, data: u8) {
        if addr < 0x2000 || addr >= 0x4000 {
            panic!("read register addr {:} is out of range", addr);
        }
        self.previous_write_data = data;
        match addr % 8 {
           0 => {
               self.write_ctrl(data);
           },
           1 => {
               self.write_mask(data);
           },
           2 => {
               // TODO do nothing
           }
           3 => {
               self.write_oam_addr(data);
           },
           4 => {
               self.write_oam_data(data);
           }
           5 => {
               self.write_scroll(data);
           },
           6 => {
               self.write_addr(data);
           },
           7 => {
               self.write_data(data);
           },
           _ => {
               println!("trying to write to {:}", addr);
               panic!("unimplemented")
           },
        };
    }

    // write $2000: PPUCTRL
    fn write_ctrl(&mut self, data: u8) {
        self.base_nametable_addr_flag = data & 0x03;
        self.vram_increment_flag = (data >> 2) & 1;
        self.sprite_table_addr_flag = (data >> 3) & 1;
        self.background_table_addr_flag = (data >> 4) & 1;
        self.sprite_size_flag = (data >> 5) & 1;
        self.master_slave_flag = (data >> 6) & 1;
        self.nmi_output = (data >> 7) & 1 == 1;

        self.t = self.t &0xF3FF | ((data as u16 & 0x0003) << 10);
    }

    // write $2001: PPUMASK
    fn write_mask(&mut self, data: u8) {
        self.greyscale_flag = data & 1;
        self.show_left_background_flag = (data >> 1) & 1;
        self.show_left_sprites_flag = (data >> 2) & 1;
        self.show_background_flag = (data >> 3) & 1;
        self.show_sprites_flag = (data >> 4) & 1;
        self.emphasize_red_flag = (data >> 5) & 1;
        self.emphasize_green_flag = (data >> 6) & 1;
        self.emphasize_blue_flag = (data >> 7) & 1;
    }

    // write $2003: OAMADDR
    fn write_oam_addr(&mut self, data: u8) {
        self.oam_addr = data;
    }

    // write $2004: Write OAM data
    fn write_oam_data(&mut self, data: u8) {
        self.oam_data[self.oam_addr as usize] = data;
        self.oam_addr += 1;
    }

    // write $2005: PPUSCROLL
    fn write_scroll(&mut self, data: u8) {
        if self.w == 0 {
            self.t = self.t & 0xFFE0 | ((data >> 3) as u16);
            self.x = data & 0x07;
            self.w = 1;
        } else {
            self.t = self.t & 0x8FFF | (((data & 0x07) as u16) << 12);
            self.t = self.t & 0xFC1F | (((data & 0xF8) as u16) << 2);
            self.w = 0;
        }
    }

    // write $2006: PPUADDR
    fn write_addr(&mut self, data: u8) {
        if self.w == 0 {
            self.t = self.t & 0x80FF | (((data & 0x3F) as u16) << 8);
            self.w = 1;
        } else {
            self.t = self.t & 0xFF00 | data as u16;
            self.v = self.t;
            self.w = 0;
        }
    }

    // write $2007: PPUDATA
    fn write_data(&mut self, data: u8) {
        if self.v < 0x2000 {
            self.mem.write(self.v, data);
        } else if self.v < 0x3000 {
            let address = self.mem.get_nametable_index(self.v);
            self.nametable_data[address as usize] = data;
        } else if self.v >= 0x3F00 && self.v <= 0x3FFF {
            let mut address = 0x3F00 + (self.v % 0x20);
            if address >= 0x3F10 && address%4 == 0 {
                address = address - 16;
            }
            self.palette_data[(address % 0x20) as usize] = data;
        } else {
            unimplemented!();
        }

        if self.vram_increment_flag == 0 {
            self.v += 1;
        } else {
            self.v += 32;
        }
    }

    // write $4014: OAMDMA
    pub fn write_oamdma(&mut self, oam_data: [u8; 256]) {
        self.oam_data = oam_data;
        let mut i = 1;
        while i <=256 {
            i += 4;
        }
        self.oam_addr += 255;
    }

    // read $2007: PPUDATA
    fn read_data(&mut self) -> u8 {
        let mut data;
        if self.v < 0x2000 {
            data = self.mem.read(self.v);
        } else if self.v < 0x3000 {
            let address = self.mem.get_nametable_index(self.v);
            data = self.nametable_data[address as usize];
        } else if self.v >= 0x3F00 && self.v <= 0x3FFF {
            let mut address = 0x3F00 + (self.v % 0x20);
            if self.v >= 0x3F10 && self.v%4 == 0 {
                address = self.v - 16;
            }
            data = self.palette_data[(address % 0x20) as usize]
        } else {
            // Gauntlet for some reason reads from memory address 0x4000.
            // This gets us past that issue.
            data = 0;
        }
        if self.v <= 0x3EFF {
            let buffer_data = self.ppu_data_buffer;
            self.ppu_data_buffer = data;
            data = buffer_data;
        }

        if self.vram_increment_flag == 0 {
            self.v += 1;
        } else {
            self.v += 32;
        }
        return data;
    }

    // read $2002 PPUSTATUS
    fn read_status(&mut self) -> u8 {
        let mut status: u8 = self.previous_write_data & 0x1F;
        status |= (self.sprite_overflow_flag as u8) << 5;
        status |= (self.sprite_zero_hit_flag as u8) << 6;
        if self.nmi_occurred {
            status |= 1 << 7;
        }
        self.nmi_occurred = false;
        self.w = 0;
        return status;
    }

    // read $2004: Read OAM data
    fn read_oam_data(&mut self) -> u8 {
        return self.oam_data[self.oam_addr as usize];
    }

    // copy_horizontal bits from t to v.
    fn copy_horizontal(&mut self) {
        self.v = (self.v & 0xFBE0) | (self.t & 0x041F);
    }

    // copy_vertical bits from t to v.
    fn copy_vertical(&mut self) {
        // v: IHGF.ED CBA..... = t: IHGF.ED CBA.....
        self.v = (self.v & 0x041F) | (self.t & 0x7BE0);
    }

    // Increment coarse x after every tile.
    // Wrapping around if we are at the end of a line.
    fn increment_x_coarse(&mut self) {
        if self.v & 0x001F == 31 {
            self.v = self.v & 0xFFE0;
            self.v = self.v ^ 0x0400;
        } else {
            self.v += 1;
        }
    }

    fn increment_y(&mut self) {
        if self.v & 0x7000 != 0x7000 {
            self.v += 0x1000;
        } else {
            self.v = self.v & 0x0FFF;
            let mut coarse_y = (self.v & 0x03E0) >> 5;
            if coarse_y == 29 {
                coarse_y = 0;
                self.v = self.v ^ 0x0800;
            } else if coarse_y == 31 {
                coarse_y = 0;
            } else {
                coarse_y += 1;
            }
            self.v = (self.v & 0x7C1F) | (coarse_y << 5);
        }
    }

    fn get_nametable_byte(&mut self) {
        let mut address = 0x2000 | (self.v & 0x0FFF);
        address = self.mem.get_nametable_index(address);
        self.nametable_byte = self.nametable_data[address as usize];
    }

    fn get_attribute_byte(&mut self) {
        let mut address = 0x23C0 | (self.v & 0x0C00) | ((self.v >> 4) & 0x38) | ((self.v >> 2) & 0x07);
        address = self.mem.get_nametable_index(address);
        self.attribute_byte = self.nametable_data[address as usize];
    }

    fn get_low_bg_tile_byte(&mut self) {
        let fine_y = self.v >> 12;
        let mut background_table_address: u16 = 0x0000;
        if self.background_table_addr_flag == 1 {
            background_table_address = 0x1000;
        }
        let address = background_table_address + ((self.nametable_byte as u16) * 16) + fine_y;
        self.low_bg_tile_byte = self.mem.read(address);
    }

    fn get_high_bg_tile_byte(&mut self) {
        let fine_y = self.v >> 12;
        let mut background_table_address: u16 = 0x0000;
        if self.background_table_addr_flag == 1 {
            background_table_address = 0x1000;
        }
        let address = background_table_address + ((self.nametable_byte as u16) * 16) + fine_y;
        self.high_bg_tile_byte = self.mem.read(address+8);
    }

    fn background_pixel(&mut self) -> u8 {
        if self.show_background_flag == 0 {
            return 0;
        }
        let bg_bits = ((((self.high_bit_bitmap_bg_shift_register << self.x) & 0x8000) >> 14) | ((self.low_bit_bitmap_bg_shift_register << self.x) & 0x8000) >> 15) as u8;
        let attr_bits = (((((self.high_bit_palette_attr_bg_shift_register << self.x) & 0x8000) >> 14) | ((self.low_bit_palette_attr_bg_shift_register << self.x) & 0x8000) >> 15) << 2) as u8;
        let pixel = attr_bits | bg_bits;
        return pixel;
    }

    // returns sprite_index, sprite_priotity, sprite_pixel 
    fn sprite_pixel(&mut self) -> (u8, u8, u8) {
        if self.show_sprites_flag == 0 {
            return (0, 0, 0);
        }
        for i in 0..self.sprite_count {
            let x_range = (self.cycle as i32 - 1) - self.sprite_positions[i as usize] as i32;
            if x_range >= 0 && x_range <=7 {
                let pattern_bits = ((self.high_bit_sprite_bitmaps[i as usize] >> (7 - x_range) & 1) << 1) | ((self.low_bit_sprite_bitmaps[i as usize] >> (7 - x_range) & 1));
                let attr_bits = (self.sprite_attributes[i as usize] & 0x03) << 2;
                let pixel = attr_bits | pattern_bits;
                if pixel%4 == 0 {
                    continue;
                }
                let priority = (self.sprite_attributes[i as usize] >> 5) & 1;
                return (i, priority, pixel);
            }
        }
        return (0, 0, 0);
    }

    fn get_sprite_height(&mut self) -> u16 {
        if self.sprite_size_flag == 0 {
            8
        } else {
            16
        }
    }

    fn load_next_scaline_sprites(&mut self) {
        let sprite_height = self.get_sprite_height();
        let mut sprite_count = 0;

        for n in 0..64 {
            let y = self.oam_data[n*4];
            let attributes = self.oam_data[(n*4) + 2];
            let x = self.oam_data[(n*4) + 3];
            let row = self.scanline.wrapping_sub(y as u16);
            if row >= sprite_height {
                continue
            }
            if sprite_count < 8 {
                self.sprite_positions[sprite_count] = x;
                self.sprite_attributes[sprite_count] = attributes;
                self.sprite_indexes[sprite_count] = n as u8;
                if sprite_height == 8 {
                    let tile_number = self.oam_data[(n*4) + 1];
                    let mut sprite_pattern_table_address: u16 = 0x0000;
                    if self.sprite_table_addr_flag == 1 {
                        sprite_pattern_table_address = 0x1000;
                    }
                    let mut tile_row = row;
                    // Flip vertically.
                    if (attributes & 0x80) == 0x80 {
                        tile_row = 7 - tile_row;
                    }
                    let address = sprite_pattern_table_address + (tile_number as u16 * 16) + tile_row;
                    let mut low_bits = self.mem.read(address);
                    let mut high_bits = self.mem.read(address+8);
                    // Flip horizontally.
                    if (attributes & 0x40) == 0x40 {
                        low_bits = horizontally_flip_bits(low_bits);
                        high_bits = horizontally_flip_bits(high_bits);
                    }
                    self.low_bit_sprite_bitmaps[sprite_count] = low_bits;
                    self.high_bit_sprite_bitmaps[sprite_count] = high_bits;
                } else {
                    let tile_number_all = self.oam_data[(n*4) + 1];
                    let pattern_bank = ((tile_number_all & 1) as u16) << 12;
                    let mut tile_row = row;
                    // Flip vertically.
                    if (attributes & 0x80) == 0x80 {
                        tile_row = 15 - tile_row;
                    }
                    let mut tile_number = tile_number_all & 0xFE;
                    if tile_row > 7 {
                        tile_number += 1;
                        if tile_row > 7 {
                            tile_row -= 8;
                        }
                    }
                    let address = pattern_bank + (tile_number as u16 * 16) + tile_row;
                    let mut low_bits = self.mem.read(address);
                    let mut high_bits = self.mem.read(address+8);
                    if (attributes & 0x40) == 0x40 {
                        low_bits = horizontally_flip_bits(low_bits);
                        high_bits = horizontally_flip_bits(high_bits);
                    }
                    self.low_bit_sprite_bitmaps[sprite_count] = low_bits;
                    self.high_bit_sprite_bitmaps[sprite_count] = high_bits;
                }
            }
            sprite_count += 1;
        }
        if sprite_count > 8 {
            sprite_count = 8;
            self.sprite_overflow_flag = true;
        }
        self.sprite_count = sprite_count as u8;
    }

    fn render_pixel(&mut self) {
        let x = self.cycle - 1;
        let y = self.scanline;

        let mut bg_pixel = self.background_pixel();
        let (sprite_index, sprite_priority, mut sprite_pixel) = self.sprite_pixel();
        if x < 8 && self.show_left_background_flag == 0 {
            bg_pixel = 0;
        }
        if x < 8 && self.show_left_sprites_flag == 0 {
            sprite_pixel = 0;
        }
        if bg_pixel % 4 == 0 && sprite_pixel % 4 == 0 {
            self.frame_buffer.put_pixel(x as u32, y as u32, Rgb{
                data: PALETTE[self.palette_data[0] as usize],
            });
        } else if bg_pixel % 4 == 0 && sprite_pixel % 4 != 0 {
            self.frame_buffer.put_pixel(x as u32, y as u32, Rgb{
                data: PALETTE[self.palette_data[(sprite_pixel | 0x10) as usize] as usize],
            });
        } else if bg_pixel % 4 != 0 && sprite_pixel % 4 == 0 {
            self.frame_buffer.put_pixel(x as u32, y as u32, Rgb{
                data: PALETTE[self.palette_data[bg_pixel as usize] as usize],
            });
        } else {
            if self.sprite_indexes[sprite_index as usize] == 0 && x != 255 {
                self.sprite_zero_hit_flag = true;
            }
            if sprite_priority == 0 {
                self.frame_buffer.put_pixel(x as u32, y as u32, Rgb{
                    data: PALETTE[self.palette_data[(sprite_pixel | 0x10) as usize] as usize],
                });
            } else {
                self.frame_buffer.put_pixel(x as u32, y as u32, Rgb{
                    data: PALETTE[self.palette_data[bg_pixel as usize] as usize],
                });
            }
        }
    }

    // step the ppu one cycle.
    pub fn step(&mut self) -> StepOutput {
        let mut step_output = StepOutput{
            nmi: false,
            frame_change: false,
        };

        if self.show_background_flag == 1 || self.show_sprites_flag == 1 {
            if self.scanline < 240 && self.cycle >= 1 && self.cycle <= 256 {
                self.render_pixel();
            }

            if (self.scanline == 261 || self.scanline < 240) && ((self.cycle >= 1 && self.cycle <= 256) || (self.cycle >= 321 && self.cycle <= 336)) {
                self.low_bit_bitmap_bg_shift_register <<= 1;
                self.high_bit_bitmap_bg_shift_register <<= 1;
                self.low_bit_palette_attr_bg_shift_register <<= 1;
                self.high_bit_palette_attr_bg_shift_register <<= 1;
                match self.cycle%8 {
                    0 => {
                        self.low_bit_bitmap_bg_shift_register = (self.low_bit_bitmap_bg_shift_register & 0xFF00) | (self.low_bg_tile_byte as u16);

                        self.high_bit_bitmap_bg_shift_register = (self.high_bit_bitmap_bg_shift_register & 0xFF00) | (self.high_bg_tile_byte as u16);

                        let attr_bits = (self.attribute_byte << (6 - (((self.v >> 4) & 4) | self.v & 2))) >> 6;
                        if attr_bits & 0x01 == 0x01 {
                            self.low_bit_palette_attr_bg_shift_register = (self.low_bit_palette_attr_bg_shift_register & 0xFF00) | 0xFF;
                        } else {
                            self.low_bit_palette_attr_bg_shift_register = (self.low_bit_palette_attr_bg_shift_register & 0xFF00) | 0x00;
                        }
                        if attr_bits & 0x02 == 0x02 {
                            self.high_bit_palette_attr_bg_shift_register = (self.high_bit_palette_attr_bg_shift_register & 0xFF00) | 0xFF;
                        } else {
                            self.high_bit_palette_attr_bg_shift_register = (self.high_bit_palette_attr_bg_shift_register & 0xFF00) | 0x00;
                        } 
                    },
                    1 => {
                        self.get_nametable_byte();
                    },
                    3 => {
                        self.get_attribute_byte();
                    },
                    5 => {
                        self.get_low_bg_tile_byte();
                    },
                    7 => {
                        self.get_high_bg_tile_byte();
                    }
                    _ => {
                        // Do nothing
                    }
                };
            }

            if (self.scanline == 261 || self.scanline < 240) && self.cycle == 257 {
                self.copy_horizontal();
            }

            if (self.scanline == 261 || self.scanline < 240) && self.cycle == 256 {
                self.increment_y();
            }

            if (self.scanline == 261 || self.scanline < 240) && ((self.cycle >= 1 && self.cycle <= 256) || (self.cycle >= 328 && self.cycle <= 336)) && self.cycle%8 == 0 {
                self.increment_x_coarse();
            }

            if self.scanline == 261 && self.cycle >= 280 && self.cycle <= 304 {
                self.copy_vertical();
            }

            if self.cycle == 257 {
                if self.scanline < 240 {
                    self.load_next_scaline_sprites();
                } else {
                    self.sprite_count = 0;
                }
            }
        }


        if self.cycle == 1 && self.scanline == 241 {
            self.nmi_occurred = true;
            step_output.frame_change = true;
            if self.nmi_occurred && self.nmi_output {
                step_output.nmi = true;
            }
        }

        if self.cycle == 1 && self.scanline == 261 {
            self.nmi_occurred = false;
            self.sprite_zero_hit_flag = false;
            self.sprite_overflow_flag = false;
        }

        // Move cycle and scanline forward
        self.cycle += 1;
        if self.cycle > 340 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
            }
        }

        return step_output;
    }

    pub fn cycle(&self) -> u16 {
        return self.cycle;
    }

    pub fn scanline(&self) -> u16 {
        return self.scanline;
    }

    pub fn get_show_background_flag(&self) -> bool {
        return self.show_background_flag == 1;
    }

    pub fn get_show_sprite_flag(&self) -> bool {
        return self.show_sprites_flag == 1;
    }
}

fn horizontally_flip_bits(num: u8) -> u8 {
    let mut flipped_num = 0;
    for i in 0..8 {
        flipped_num = flipped_num | (((num & (1 << i)) >> i) << (7 - i));
    }
    return flipped_num;
}

pub struct PPUMemory {
    pub mapper: Rc<RefCell<Box<dyn mapper::Mapper>>>,
    pub nametable_mirror: Rc<RefCell<Box<NametableMirroring>>>,
}

impl Memory for PPUMemory {
    fn read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            return self.mapper.borrow_mut().read(addr);
        } else {
            unimplemented!();
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        if addr < 0x2000 {
            self.mapper.borrow_mut().write(addr, data);
        } else {
            unimplemented!();
        }
    }

    fn get_nametable_index(&mut self, address: u16) -> u16 {
        let table_num = (address - 0x2000) / 0x0400;
        let table_offset = (address - 0x2000) % 0x0400;
        let table_index;
        let nametable_mirror_type = self.nametable_mirror.borrow_mut().get_nametable_mirror_type();
        match nametable_mirror_type {
            NametableMirrorType::Horizontal => {
                if table_num == 0 || table_num == 1 {
                    table_index = 0;
                } else {
                    table_index = 1;
                }
            },
            NametableMirrorType::Vertical => {
                if table_num == 0 || table_num == 2 {
                    table_index = 0;
                } else {
                    table_index = 1;
                }
            },
            NametableMirrorType::Single0 => {
                table_index = 0;
            },
            NametableMirrorType::Single1 => {
                table_index = 1;
            },
            NametableMirrorType::Four => {
                table_index = table_num;
            }
        }
        return (table_index*1024)+table_offset;
    }
}
