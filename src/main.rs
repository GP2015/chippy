#![warn(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::unreadable_literal)]

mod config;
mod cpu;
mod emulib;
mod gpu;
mod input;
mod instructions;
mod ram;
mod timer;
mod window;

use crate::cpu::Cpu;
use crate::gpu::Gpu;
use crate::input::InputManager;
use crate::ram::Ram;
use crate::timer::{DelayTimer, SoundTimer};
use crate::window::WindowManager;
use clap::Parser;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use winit::event_loop::{ControlFlow, EventLoop};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    program_path: String,
}

struct Components {
    active: Arc<AtomicBool>,
    cpu: Arc<Cpu>,
    gpu: Arc<Gpu>,
    ram: Arc<Ram>,
    delay_timer: Arc<DelayTimer>,
    sound_timer: Arc<SoundTimer>,
    input_manager: Arc<InputManager>,
}

fn main() {
    println!("Starting emulator...");

    let args = Args::parse();

    let Some(comps) = create_components() else {
        println!("Stopping emulator...");
        return;
    };

    comps.ram.load_program(&args.program_path);

    let mut window_manager = WindowManager::new(
        comps.active.clone(),
        comps.gpu.clone(),
        comps.input_manager.clone(),
    );

    let event_loop = match EventLoop::new() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to create event loop ({e}).");
            return;
        }
    };

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handles = Vec::new();

    handles.push(thread::spawn(move || comps.delay_timer.run()));
    handles.push(thread::spawn(move || comps.sound_timer.run()));

    if comps.gpu.should_render_separately() {
        handles.push(thread::spawn(move || comps.gpu.run_separate_render()));
    }

    handles.push(thread::spawn(move || comps.cpu.run()));

    if let Err(e) = event_loop.run_app(&mut window_manager) {
        eprintln!("Window manager event loop failed ({e}).");
        comps.active.store(false, Ordering::Release);
    }

    if cfg!(debug_assertions) {
        assert!(
            !comps.active.load(Ordering::Relaxed),
            "Event loop should not have exited while active is high."
        );
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Stopping emulator...");
}

fn create_components() -> Option<Components> {
    let config = config::generate_configs()?;
    let active = Arc::new(AtomicBool::new(true));
    let delay_timer = DelayTimer::try_new(active.clone(), config.delay_timer)?;
    let sound_timer = SoundTimer::try_new(active.clone(), config.sound_timer)?;
    let input_manager = InputManager::try_new(active.clone(), config.input);
    let ram = Ram::try_new(active.clone(), config.ram)?;
    let gpu = Gpu::try_new(active.clone(), config.gpu)?;
    let cpu = Cpu::try_new(
        active.clone(),
        config.cpu,
        gpu.clone(),
        ram.clone(),
        delay_timer.clone(),
        sound_timer.clone(),
        input_manager.clone(),
    )?;

    Some(Components {
        active,
        cpu,
        gpu,
        ram,
        delay_timer,
        sound_timer,
        input_manager,
    })
}
