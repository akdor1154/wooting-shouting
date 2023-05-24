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
mod watcher;

fn main() {
    let mut reader = hid::WootingPlugin::new();
    let cb = |ev: wooting::DeviceEventType, info: &wooting::DeviceInfo| {};
    let res = reader.initialise(Box::new(cb));
    let Ok((l, rx)) = res.0 else {
        panic!("failed to init")
    };

    let mut watcher = watcher::KeyWatcher::new();

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
