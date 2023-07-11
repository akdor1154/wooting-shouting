use std::collections::HashMap;

use crate::hid;

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
		current_time: std::time::Instant,
	},
	PressFired,
}

pub struct KeyWatcher {
	keys: HashMap<u16, KeyState>,
	tx: std::sync::mpsc::SyncSender<KeyEvent>,
}

pub struct KeyEvent {
	pub scancode: u16,
	pub caps: bool,
	pub velocity: f32,
}

const THRESHOLD_LOW: f32 = 0.4;
const THRESHOLD: f32 = 0.92;

impl KeyWatcher {
	pub fn new(tx: std::sync::mpsc::SyncSender<KeyEvent>) -> Self {
		return Self {
			keys: HashMap::<_, _>::with_capacity(255),
			tx: tx,
		};
	}
	fn get_key_state(&mut self, code: u16) -> &mut KeyState {
		if !self.keys.contains_key(&code) {
			self.keys.insert(code, KeyState::Released);
		}
		return self.keys.get_mut(&code).unwrap();
	}

	pub fn take_input(&mut self, input: &hid::AnalogueReading) {
		let hid::AnalogueReading {
			scancode: code,
			value,
			ts,
		} = input;
		let tx = &self.tx.clone();
		let s = self.get_key_state(*code);

		//let code = key_id.to_u16().expect("Failed to convert HIDCode to u16");
		//let value = analog_data.get(&code).unwrap_or(&0.0);

		//info!("val for {code} is {value}");
		match (&s, *value > 0.0) {
			(
				KeyState::PressStarted {
					start_time,
					current_value,
					current_time,
				},
				_,
			) => {
				// started release
				let diff = *value - *current_value;
				if *value > THRESHOLD // key nearly fully depressed
				|| diff < 0.0 && *value > THRESHOLD_LOW
				// started release
				// key has begun to be depressed
				{
					let last_value = *current_value;
					let last_time = *current_time;

					let tdiff = *ts - *start_time;
					//let tdiff = *ts - last_time;

					let velocity = (*value - 0.0) / tdiff.as_secs_f32();

					tx.send(KeyEvent {
						scancode: *code,
						caps: (velocity > 180.0),
						velocity,
					})
					.unwrap();
					*s = KeyState::PressFired
				} else {
					*s = KeyState::PressStarted {
						start_time: *start_time,
						current_value: *value,
						current_time: *ts,
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
					start_time: *ts,
					current_value: *value,
					current_time: *ts,
				};
			}
			(KeyState::Released, false) => {
				*s = KeyState::Released;
			}
		}
	}
}
