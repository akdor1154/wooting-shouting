//extern crate wooting_analog_wrapper;
use wooting_analog_plugin_dev::wooting_analog_common as wooting;

//use wooting_analog_wrapper as sdk;

//pub use sdk::{DeviceInfo, FromPrimitive, HIDCodes, ToPrimitive, WootingAnalogResult};

use std::{
	collections::{HashSet},
	thread,
};
//use sdk::SDKResult;
use env_logger;
use log::*;

mod hid;
mod keycode;
mod outputhid;
mod watcher;
mod recorder;

const READ_CHANNEL_BUF_SIZE: usize = 128;
const OUT_CHANNEL_BUF_SIZE: usize = 8;
const RECORD_CHANNEL_BUF_SIZE: usize = 64;


fn main() {
	env_logger::init();

	let (hid_tx, in_rx) = std::sync::mpsc::sync_channel::<hid::Input>(READ_CHANNEL_BUF_SIZE);
	let (ev_tx, ev_rx) =
		std::sync::mpsc::sync_channel::<OutputHidEvent>(OUT_CHANNEL_BUF_SIZE);
	let (record_tx, record_rx) =
		std::sync::mpsc::sync_channel::<hid::AnalogueReading>(RECORD_CHANNEL_BUF_SIZE);



	let mut watcher = watcher::KeyWatcher::new(ev_tx.clone());

	let mut last_pressed = HashSet::<u16>::new();

	let mut reader = hid::WootingPlugin::new(hid_tx.clone());

	{
		let cb = |ev: wooting::DeviceEventType, info: &wooting::DeviceInfo| {};
		let res = reader.initialise(Box::new(cb));
		let Ok(_) = res.0 else {
			panic!("failed to init")
		};


		ctrlc::set_handler(move || {
			info!("got handler!");
			info!("unloaded!");
			hid_tx.send(hid::Input::Fin()).unwrap();
		}).unwrap();

	}


	let t_in = {
		thread::spawn(move || {
			for input in in_rx {
				match input {
					hid::Input::Analogue(kk) => {
						let mut pressed = HashSet::<u16>::new();
						for input in &kk {
							pressed.insert(input.scancode);
							watcher.take_input(input);
							record_tx.send(input.to_owned()).unwrap();
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
						ev_tx.send(OutputHidEvent::Passthrough(evs)).unwrap();
					},
					hid::Input::Fin() => {
						return;
					}
				}
			}
			info!("closing in_rx watcher");
		})
	};

	let t_out = {
		thread::spawn(move || {
			let mut outputhid = outputhid::OutputHid::new();
			for input in ev_rx {
				match input {
					OutputHidEvent::Key(k) => outputhid.send_key(&k),
					OutputHidEvent::Passthrough(evs) => outputhid.send_passthrough(&evs),
				}
			}
			info!("closing ev_rx watcher");
		})
	};

	let rec_in = {
		thread::spawn(|| {
			let con = recorder::sqlite_connection().unwrap();
			let mut recorder = recorder::Recorder::new(&con);
			for input in record_rx {
				recorder.record(&input);
			}
			info!("closing rec_in watcher");
		})
	};

	t_in.join().unwrap();

	t_out.join().unwrap();

	rec_in.join().unwrap();

	info!("closing main");
	reader.unload();
}


pub enum OutputHidEvent {
	Key(watcher::KeyEvent),
	Passthrough(Vec<input_linux::sys::input_event>)
}

// rakers
// anvil
// laceys
