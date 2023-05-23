
//extern crate wooting_analog_wrapper;
use wooting_analog_plugin_dev::wooting_analog_common as wooting;

use serde::{Serialize, Deserialize};

//use wooting_analog_wrapper as sdk;

//pub use sdk::{DeviceInfo, FromPrimitive, HIDCodes, ToPrimitive, WootingAnalogResult};

use std::{collections::{HashMap, HashSet}, thread, time::Duration};
use anyhow::{Context, Result};
//use sdk::SDKResult;
use log::*;
use env_logger;

mod hid;

struct KeyState {
    press_out_started: bool,
    press_out_fired: bool,
    press_in_start_time: std::time::Instant,
    press_in_value: f32
}

impl KeyState {
    fn new() -> Self {
        return Self {
            press_out_started: false,
            press_out_fired: false,
            press_in_start_time: std::time::Instant::now(),
            press_in_value: 0.0
        }
    }
}
struct KeyWatcher {
    keys: HashMap<u16, KeyState>
}

impl KeyWatcher {
    fn new() -> Self {
        return Self {
            keys: HashMap::<_,_>::with_capacity(255)
        }
    }
    fn get_key_state(&mut self, code: u16) -> &mut KeyState {
        if !self.keys.contains_key(&code) {
            self.keys.insert(code, KeyState::new());
        }
        return self.keys.get_mut(&code).unwrap();
    }

    fn take_input(&mut self, input: &hid::ReadKey) {
        let (code, value) = input;
        let mut s = self.get_key_state(*code);

        //let code = key_id.to_u16().expect("Failed to convert HIDCode to u16");
        //let value = analog_data.get(&code).unwrap_or(&0.0);

        //info!("val for {code} is {value}");
        match (s.press_out_started, s.press_out_fired, *value > 0.0) {
            (true, false, _) => {
                let last_value = s.press_in_value;
                let diff = value - last_value;
                let now = std::time::Instant::now();
                let tdiff = now - s.press_in_start_time;
                let diffbyt = diff/tdiff.as_secs_f32();
                if diff != 0.0 {
                    info!("diff for {code} is {diffbyt}");
                    s.press_out_started = true;
                    s.press_out_fired = true;
                    s.press_in_start_time = s.press_in_start_time;
                    s.press_in_value = *value;
                } else {
                    s.press_out_started = true;
                    s.press_out_fired = false;
                    s.press_in_start_time = s.press_in_start_time;
                    s.press_in_value = *value;
                }
            },
            (true, true, true) => {
                s.press_in_value = *value;
            },
            (true, true, false) => {
                s.press_out_started = false;
                s.press_out_fired = false;
                s.press_in_start_time = s.press_in_start_time;
                s.press_in_value = *value;
            },
            (false, _, true) => {
                s.press_out_started = true;
                s.press_out_fired = false;
                s.press_in_start_time = std::time::Instant::now();
                s.press_in_value = *value;
            },
            (false, _, false) => {
                s.press_out_started = false;
                s.press_out_fired = false;
                s.press_in_start_time = s.press_in_start_time;
                s.press_in_value = *value;
            }
        }
    }
}

fn main() {
    let mut reader = hid::WootingPlugin::new();
    let cb = |ev: wooting::DeviceEventType, info: &wooting::DeviceInfo| { };
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
