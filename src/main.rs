//extern crate wooting_analog_wrapper;
use wooting_analog_plugin_dev::wooting_analog_common as wooting;

use serde::{Deserialize, Serialize};

//use wooting_analog_wrapper as sdk;

//pub use sdk::{DeviceInfo, FromPrimitive, HIDCodes, ToPrimitive, WootingAnalogResult};

use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet},
    thread,
    time::Duration,
};
//use sdk::SDKResult;
use env_logger;
use log::*;

mod hid;

// struct KeyState {
//     press_out_started: bool,
//     press_out_fired: bool,
//     press_in_start_time: std::time::Instant,
//     press_in_value: f32,
// }
enum KeyState {
    Released,
    PressStarted {
        start_time: std::time::Instant,
        current_value: f32,
    },
    PressFired,
}

struct KeyWatcher {
    keys: HashMap<u16, KeyState>,
}

const THRESHOLD: f32 = 0.5;

impl KeyWatcher {
    fn new() -> Self {
        return Self {
            keys: HashMap::<_, _>::with_capacity(255),
        };
    }
    fn get_key_state(&mut self, code: u16) -> &mut KeyState {
        if !self.keys.contains_key(&code) {
            self.keys.insert(code, KeyState::Released);
        }
        return self.keys.get_mut(&code).unwrap();
    }

    fn take_input(&mut self, input: &hid::ReadKey) {
        let (code, value) = input;
        let s = self.get_key_state(*code);

        //let code = key_id.to_u16().expect("Failed to convert HIDCode to u16");
        //let value = analog_data.get(&code).unwrap_or(&0.0);

        //info!("val for {code} is {value}");
        match (&s, *value > 0.0) {
            (
                KeyState::PressStarted {
                    start_time,
                    current_value,
                },
                _,
            ) => {
                // started release
                if *value > THRESHOLD {
                    let now = std::time::Instant::now();
                    let tdiff = now - *start_time;

                    let last_value = *current_value;

                    let diff = (*value - last_value) / tdiff.as_secs_f32();

                    info!("diff for {code} is {diff}");
                    *s = KeyState::PressFired
                } else {
                    *s = KeyState::PressStarted {
                        start_time: *start_time,
                        current_value: *value,
                    };
                }
            }
            (KeyState::PressFired, true) => {
                //*s = *s;
            }
            (KeyState::PressFired, false) => {
                *s = KeyState::Released;
            }
            (KeyState::Released, true) => {
                *s = KeyState::PressStarted {
                    start_time: std::time::Instant::now(),
                    current_value: *value,
                };
            }
            (KeyState::Released, false) => {
                *s = KeyState::Released;
            }
        }
    }
}

fn main() {
    let mut reader = hid::WootingPlugin::new();
    let cb = |ev: wooting::DeviceEventType, info: &wooting::DeviceInfo| {};
    let res = reader.initialise(Box::new(cb));
    let Ok((l, rx)) = res.0 else {
        panic!("failed to init")
    };

    let mut watcher = KeyWatcher::new();

    let mut last_pressed = HashSet::<u16>::new();

    for kk in rx {
        let mut pressed = HashSet::<u16>::new();
        for input in &kk {
            pressed.insert(input.0);
            watcher.take_input(input);
            //info!("got {code}:{analog}")
        }
        let missing_this_time = last_pressed.difference(&pressed);
        for code in missing_this_time {
            watcher.take_input(&(*code, 0.0f32))
        }
        last_pressed = pressed;
    }
}
