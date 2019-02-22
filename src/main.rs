extern crate image;
extern crate sdl2;
extern crate portaudio;
extern crate time;
extern crate clap;

mod ines;
mod cpu;
mod ppu;
mod apu;
mod controller;
mod mapper;
mod nes;

use sdl2::pixels::PixelFormatEnum;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use std::sync::mpsc;
use clap::{Arg, App};
use controller::Buttons;

const CHANNELS: i32 = 1;
const FRAMES_PER_BUFFER: u32 = 2;
const CPU_FREQUENCY: u32 = 1789773;
const KEYBOARD_REFRESH_RATE: u32 = 120;

fn main() {
    let matches = App::new("nese")
                          .author("Matt McCoy <mattnenterprise@yahoo.com>")
                          .arg(Arg::with_name("filename")
                                        .help("the game file to use")
                                        .index(1)
                                        .required(true)
                          ).get_matches();
    
    let game_file = matches.value_of("filename").unwrap();

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("nese", 256, 240)
        .position_centered()
        .opengl()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let mut texture = texture_creator.create_texture_streaming(
        PixelFormatEnum::RGB24, 256, 240).unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    let (audio_sample_sender, audio_sample_receiver) = mpsc::sync_channel::<f32>(44100);
    let pa = portaudio::PortAudio::new().unwrap();
    let mut settings = pa.default_output_stream_settings(CHANNELS, 44100.0, FRAMES_PER_BUFFER).unwrap();
    // we won't output out of range samples so don't bother clipping them.
    settings.flags = portaudio::stream_flags::CLIP_OFF;
    let port_audio_callback = move |portaudio::OutputStreamCallbackArgs { buffer, frames, .. }| {
        let mut idx = 0;
        for _ in 0..frames {
            match audio_sample_receiver.try_recv() {
                Ok(d) => buffer[idx] = d,
                Err(_) => {
                    buffer[idx] = 0.0
                },
            }
            // TODO I had to make this non blocking so the program would properly close.
            //buffer[idx] = audio_sample_receiver.recv().unwrap();
            idx += 1;
        }
        portaudio::Continue
    };
    let mut stream = pa.open_non_blocking_stream(settings, port_audio_callback).unwrap();

    let mut console = nes::Nes::new(game_file.to_string());

    let mut _total_cpu_cycles: u64 = 0;
    let mut _total_cpu_cycles_from_steps: u64 = 0;
    let mut last_keyboard_refresh_time = time::precise_time_ns();
    let mut cpu_cycle_overflow: i64 = 0;

    stream.start().unwrap();
    let mut start_time = time::precise_time_ns();
    let mut new_start_time;

    loop {
        let mut _cpu_cycles: u64 = 0;
        new_start_time = time::precise_time_ns();
        let cpu_run_time = new_start_time - start_time;
        let mut cpu_cycles_to_run = ((cpu_run_time as f64 / 1000000000.0 as f64) * CPU_FREQUENCY as f64) as i64;
        let cpu_cycles_to_run_orig = cpu_cycles_to_run;
        cpu_cycles_to_run -= cpu_cycle_overflow;
        if cpu_cycles_to_run <= 0 {
            continue;
        } else {
            let missing_time = (((cpu_run_time as f64 / 1000000000.0 as f64) * CPU_FREQUENCY as f64) - (cpu_cycles_to_run_orig as f64)) * (1.0/1789773.0) * 1000000000.0;
            start_time = new_start_time - (missing_time as u64);
            cpu_cycle_overflow = 0;
        }
       
        _total_cpu_cycles += cpu_cycles_to_run as u64;
        while cpu_cycles_to_run > 0 {
            let (step_cpu_cycles, frame_change) = console.step(&audio_sample_sender);
            _total_cpu_cycles_from_steps += step_cpu_cycles as u64;
            cpu_cycles_to_run -= step_cpu_cycles as i64;
            if frame_change {
                canvas.clear();
                let v8_pixels = console.get_frame_buffer();
                texture.update(None, v8_pixels.as_slice(), 768).unwrap();
                canvas.copy(&texture, None, None).unwrap();
                canvas.present();
            }

            _cpu_cycles += step_cpu_cycles;

            if (time::precise_time_ns() - last_keyboard_refresh_time) > ((1.0 as f64/KEYBOARD_REFRESH_RATE as f64) * 1000000000.0) as u64 {
                for event in event_pump.poll_iter() {
                    match event {
                        Event::Quit {..}
                        | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                            stream.stop().unwrap();
                            stream.close().unwrap();
                            return
                        },
                        Event::KeyDown { keycode: Some(Keycode::A), .. } => {
                            console.set_controller1_button_state(Buttons::A, true);
                        },
                        Event::KeyUp { keycode: Some(Keycode::A), .. } => {
                            console.set_controller1_button_state(Buttons::A, false);
                        },
                        Event::KeyDown { keycode: Some(Keycode::Z), .. } => {
                            console.set_controller1_button_state(Buttons::B, true);
                        },
                        Event::KeyUp { keycode: Some(Keycode::Z), .. } => {
                            console.set_controller1_button_state(Buttons::B, false);
                        },
                        Event::KeyDown { keycode: Some(Keycode::Return), .. } => {
                            console.set_controller1_button_state(Buttons::Start, true);
                        },
                        Event::KeyUp { keycode: Some(Keycode::Return), .. } => {
                            console.set_controller1_button_state(Buttons::Start, false);
                        },
                        Event::KeyDown { keycode: Some(Keycode::S), .. } => {
                            console.set_controller1_button_state(Buttons::Select, true);
                        },
                        Event::KeyUp { keycode: Some(Keycode::S), .. } => {
                            console.set_controller1_button_state(Buttons::Select, false);
                        },
                        Event::KeyDown { keycode: Some(Keycode::Left), .. } => {
                            console.set_controller1_button_state(Buttons::Left, true);
                        },
                        Event::KeyUp { keycode: Some(Keycode::Left), .. } => {
                            console.set_controller1_button_state(Buttons::Left, false);
                        },
                        Event::KeyDown { keycode: Some(Keycode::Right), .. } => {
                            console.set_controller1_button_state(Buttons::Right, true);
                        },
                        Event::KeyUp { keycode: Some(Keycode::Right), .. } => {
                            console.set_controller1_button_state(Buttons::Right, false);
                        },
                        Event::KeyDown { keycode: Some(Keycode::Up), .. } => {
                            console.set_controller1_button_state(Buttons::Up, true);
                        },
                        Event::KeyUp { keycode: Some(Keycode::Up), .. } => {
                            console.set_controller1_button_state(Buttons::Up, false);
                        },
                        Event::KeyDown { keycode: Some(Keycode::Down), .. } => {
                            console.set_controller1_button_state(Buttons::Down, true);
                        },
                        Event::KeyUp { keycode: Some(Keycode::Down), .. } => {
                            console.set_controller1_button_state(Buttons::Down, false);
                        },
                        _ => {}
                    }
                }
                last_keyboard_refresh_time = time::precise_time_ns();
            }
        }
        cpu_cycle_overflow += cpu_cycles_to_run * -1;
    }
}

