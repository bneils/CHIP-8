mod vm;

extern crate sdl2;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::event::Event;
use sdl2::keyboard::Scancode;

use std::iter::Scan;
use std::thread;
use std::time::Duration;
use vm::Env;

fn main() {
    const WIDTH: u32 = 1024; // nearest multiple of 2
    const HEIGHT: u32 = WIDTH / 2;

    let sdl_context = sdl2::init()
        .expect("Couldn't initialize SDL2");

    let video_subsystem = sdl_context
        .video()
        .expect("Couldn't initialize the video subsystem");

    let window = video_subsystem
        .window("chip8", WIDTH, HEIGHT)
        .resizable()
        .build()
        .unwrap();

    let mut event_pump = sdl_context
        .event_pump()
        .expect("Couldn't initialize the event pump. Perhaps one already exists?");

    let mut canvas = window
        .into_canvas()
        .build()
        .expect("Couldn't initialize the canvas");

    let mut env = Env::new(800);
    let mut hex_keys = [false; 16];

    'main: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,
                Event::KeyDown { scancode, .. } => {
                    if let Some(code) = scancode {
                        // I dunno if there's a better way of doing this...
                        let idx = match code {
                            Scancode::Num0 => 0,
                            Scancode::Num1 => 1,
                            Scancode::Num2 => 2,
                            Scancode::Num3 => 3,
                            Scancode::Num4 => 4,
                            Scancode::Num5 => 5,
                            Scancode::Num6 => 6,
                            Scancode::Num7 => 7,
                            Scancode::Num8 => 8,
                            Scancode::Num9 => 9,
                            Scancode::A => 10,
                            Scancode::B => 11,
                            Scancode::C => 12,
                            Scancode::D => 13,
                            Scancode::E => 14,
                            Scancode::F => 15,
                            _ => 16,
                        };
                        if idx < hex_keys.len() {
                            hex_keys[idx] = true;
                        }
                    }
                }
                _ => {},
            }
        }

        canvas.present();
        thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
