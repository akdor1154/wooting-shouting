//extern crate wooting_analog_wrapper;
use wooting_analog_plugin_dev::wooting_analog_common as wooting;

use serde::{Deserialize, Serialize};

//use wooting_analog_wrapper as sdk;

//pub use sdk::{DeviceInfo, FromPrimitive, HIDCodes, ToPrimitive, WootingAnalogResult};

use anyhow::{Context, Result};
use std::{
	collections::{HashMap, HashSet},
	sync::{Arc, Mutex},
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

const READ_CHANNEL_BUF_SIZE: usize = 64;
const OUT_CHANNEL_BUF_SIZE: usize = 8;

fn main() {
	let (hid_tx, in_rx) = std::sync::mpsc::sync_channel::<hid::Input>(READ_CHANNEL_BUF_SIZE);
	let (watch_tx, ev_rx) =
		std::sync::mpsc::sync_channel::<watcher::KeyEvent>(OUT_CHANNEL_BUF_SIZE);

	let mut reader = hid::WootingPlugin::new(hid_tx);
	let cb = |ev: wooting::DeviceEventType, info: &wooting::DeviceInfo| {};
	let res = reader.initialise(Box::new(cb));
	let Ok((l)) = res.0 else {
        panic!("failed to init")
    };
	let mut watcher = watcher::KeyWatcher::new(watch_tx);
	let outputhid = outputhid::OutputHid::new();

	let outputhid = Arc::new(Mutex::new(outputhid));

	let mut last_pressed = HashSet::<u16>::new();

	let t_in = {
		let outputhid = outputhid.clone();
		thread::spawn(move || {
			for input in in_rx {
				match input {
					hid::Input::Analogue(kk) => {
						let mut pressed = HashSet::<u16>::new();
						for input in &kk {
							pressed.insert(input.scancode);
							watcher.take_input(input);
							//info!("got {code}:{analog}")
						}
						let missing_this_time = last_pressed.difference(&pressed);
						for code in missing_this_time {
							watcher.take_input(&hid::AnalogueReading {
								scancode: *code,
								value: 0.0,
								ts: std::time::Instant::now(),
							})
						}
						last_pressed = pressed;
					}
					hid::Input::PassThrough(evs) => {
						//info!("got {evs:?}");
						outputhid.lock().unwrap().send_passthrough(&evs);
					}
				}
			}
		})
	};

	let t_out = {
		let outputhid = outputhid.clone();
		thread::spawn(move || {
			for mut k in ev_rx {
				outputhid.lock().unwrap().send_key(&k)
			}
		})
	};

	t_in.join().unwrap();

	t_out.join().unwrap();
}

// rakers
// anvil
// laceys
