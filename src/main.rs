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
mod keycode;
mod outputhid;
mod watcher;

fn main() {
	let mut reader = hid::WootingPlugin::new();
	let cb = |ev: wooting::DeviceEventType, info: &wooting::DeviceInfo| {};
	let res = reader.initialise(Box::new(cb));
	let Ok((l, in_rx)) = res.0 else {
        panic!("failed to init")
    };

	let (mut watcher, ev_rx) = watcher::KeyWatcher::new();
	let mut outputhid = outputhid::OutputHid::new();

	let mut last_pressed = HashSet::<u16>::new();

	let t_in = thread::spawn(move || {
		for kk in in_rx {
			let mut pressed = HashSet::<u16>::new();
			for input in &kk {
				pressed.insert(input.code);
				watcher.take_input(input);
				//info!("got {code}:{analog}")
			}
			let missing_this_time = last_pressed.difference(&pressed);
			for code in missing_this_time {
				watcher.take_input(&hid::ReadKey {
					code: *code,
					value: 0.0,
					ts: std::time::Instant::now(),
				})
			}
			last_pressed = pressed;
		}
	});

	let t_out = thread::spawn(move || {
		for mut k in ev_rx {
			let Some(hidcode) = keycode::hid_to_scancode(k.code) else {
				continue;
			};
			k.code = hidcode;

			outputhid.send_key(&k)
		}
	});

	t_in.join().unwrap();

	t_out.join().unwrap();
}
