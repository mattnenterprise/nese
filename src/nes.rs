extern crate time;

use apu;
use controller;
use cpu;
use ines;
use mapper;
use ppu;

use std::rc::Rc;
use std::cell::RefCell;
use std::sync::mpsc::SyncSender;

const SAMPLE_RATE: f64 = 44100.0;

pub struct Nes {
    apu: Rc<RefCell<apu::APU>>,
    ppu: Rc<RefCell<ppu::PPU<ppu::PPUMemory>>>,
    controller1: Rc<RefCell<controller::Controller>>,
    cpu: cpu::CPU<cpu::CPUMemory>,
    ppu_step_output: ppu::StepOutput,
    cpu_cycle_count: u64,
    apu_last_sample_time: u64,
    mapper: Rc<RefCell<Box<dyn mapper::Mapper>>>,
}

impl Nes {

    pub fn new(gamefile: String) -> Nes {
        let ines_data = ines::load_ines_file(&gamefile);

        let nametable_mirror_type;
        match ines_data.nametable_mirroring{
            0 => {
                nametable_mirror_type = ppu::NametableMirrorType::Horizontal;
            },
            1 => {
                nametable_mirror_type = ppu::NametableMirrorType::Vertical;
            },
            4 => {
                nametable_mirror_type = ppu::NametableMirrorType::Four;
            }
            _ => unimplemented!(),
        };
        let nametable_mirror = Box::new(ppu::NametableMirroring{
            nametable_mirror_type: nametable_mirror_type,
        });
        let rc_nametable_mirror = Rc::new(RefCell::new(nametable_mirror));

        let m: Box<dyn mapper::Mapper> = mapper::create_mapper(ines_data.mapper, ines_data.chr, ines_data.prg, Rc::clone(&rc_nametable_mirror));
        let mapper = Rc::new(RefCell::new(m));
        let ppu_memory = ppu::PPUMemory{
            mapper: Rc::clone(&mapper),
            nametable_mirror: Rc::clone(&rc_nametable_mirror),
        };

        let controller1 = Rc::new(RefCell::new(controller::Controller::new()));

        let apu = Rc::new(RefCell::new(apu::APU::new()));
        let ppu = Rc::new(RefCell::new(ppu::PPU::new(ppu_memory)));
        let memory = cpu::CPUMemory{
            mapper: Rc::clone(&mapper),
            ram: [255; 2048],
            ppu: Rc::clone(&ppu),
            apu: Rc::clone(&apu),
            controller1: Rc::clone(&controller1),
            added_stall: 0,
        };
        let cpu = cpu::CPU::new(memory);
        return Nes{
            apu: apu,
            ppu: Rc::clone(&ppu),
            controller1: Rc::clone(&controller1),
            cpu: cpu,
            ppu_step_output: ppu::StepOutput{
                nmi: false,
                frame_change: false,
            },
            cpu_cycle_count: 0,
            apu_last_sample_time: time::precise_time_ns(),
            mapper: Rc::clone(&mapper),
        };
    }

    pub fn step(&mut self, audio_sender: & SyncSender<f32>) -> (u64, bool) {
        let mut frame_change = false;
        let mut nmi = false;
        let step_cpu_cycles = self.cpu.step(self.ppu_step_output.nmi);
        self.ppu_step_output.nmi = false;
        for _ in 0..(3*step_cpu_cycles) {
            self.ppu_step_output = self.ppu.borrow_mut().step();
            if self.ppu_step_output.nmi {
                nmi = true;
            }
            if self.ppu_step_output.frame_change {
                frame_change = true;
            }
            self.mapper.borrow_mut().step(&self.ppu, &mut self.cpu);
        }
        if frame_change {
            self.ppu_step_output.frame_change = true;
        }
        if nmi {
            self.ppu_step_output.nmi = true;
        }
        for _ in 0..step_cpu_cycles {
            self.apu.borrow_mut().step(self.cpu_cycle_count, &mut self.cpu);

            let t = time::precise_time_ns();
            if (t - self.apu_last_sample_time) as f64 >= (1000000000.0 as f64/SAMPLE_RATE as f64) {
                audio_sender.send(self.apu.borrow_mut().output()).unwrap();
                self.apu_last_sample_time = t - ((t - self.apu_last_sample_time) as u64 - (1000000000.0 as f64/SAMPLE_RATE as f64) as u64);
            }

            self.cpu_cycle_count += 1;
        }
        return (step_cpu_cycles, frame_change);
    }

    pub fn set_controller1_button_state(&mut self, button: controller::Buttons, state: bool) {
        self.controller1.borrow_mut().set_button_state(button, state);
    }

    pub fn get_frame_buffer(&mut self) -> Vec<u8> {
        return self.ppu.borrow_mut().frame_buffer.clone().into_raw();
    }
}
