use std::time::Duration;

use input_linux::{uinput, InputEvent};

use crate::{hid, watcher::KeyEvent};

pub struct OutputHid {
	handle: input_linux::uinput::UInputHandle<std::fs::File>,
	epoch: std::time::Instant
}

impl OutputHid {
	pub fn new() -> Self {
		use std::os::unix::fs::OpenOptionsExt;

		let epoch = std::time::Instant::now();

		let uinput_file = std::fs::OpenOptions::new()
			.read(true)
			.write(true)
			.custom_flags(libc::O_NONBLOCK)
			.open("/dev/uinput")
			.expect("erro opening /dev/uinput");

		let handle = uinput::UInputHandle::new(uinput_file);

		handle.set_evbit(input_linux::EventKind::Key).unwrap();
		handle
			.set_evbit(input_linux::EventKind::Synchronize)
			.unwrap();

		for k in 0..248 {
			handle
				.set_keybit(input_linux::Key::from_code(k).unwrap())
				.unwrap();
		}

		let input_id = input_linux::InputId {
			bustype: input_linux::sys::BUS_USB,
			vendor: 0x4711,
			product: 0x0815,
			version: 0,
		};
		let device_name = b"Wooting SHOUTING";

		handle.create(&input_id, device_name, 0, &[]).unwrap();

		return OutputHid { handle, epoch };
	}
	pub fn send_key(&mut self, k: &KeyEvent) {
		let code = k.scancode;
		let velocity = k.velocity;
		let mut t = std::time::Instant::now().duration_since(self.epoch);
		let get_time = |t: std::time::Duration| {
			input_linux::EventTime::new(t.as_secs().try_into().unwrap(), t.subsec_millis().try_into().unwrap())
		};
		let Ok(key) = input_linux::Key::from_code(code) else {
			log::warn!("ignoring bad code {code}");
			return;
		};
		let code = Into::<u16>::into(key);

		log::info!("diff for {key:?}/{code:?} is {velocity}");

		if k.caps {
			self.handle
				.write(&[*input_linux::InputEvent::from(input_linux::KeyEvent::new(
					get_time(t),
					input_linux::Key::LeftShift,
					input_linux::KeyState::PRESSED,
				))
				.as_raw()])
				.unwrap();

			self.handle
				.write(&[
					*input_linux::InputEvent::from(input_linux::SynchronizeEvent::new(
						get_time(t),
						input_linux::SynchronizeKind::Report,
						0,
					))
					.as_raw(),
				])
				.unwrap();

			std::thread::sleep(Duration::from_millis(5));
			t = t + Duration::from_millis(5);
		}

		self.handle
			.write(&[*input_linux::InputEvent {
				time: get_time(t),
				kind: input_linux::EventKind::Key,
				code: code,
				value: 1,
			}
			.as_raw()])
			.unwrap();

		self.handle
			.write(&[
				*input_linux::InputEvent::from(input_linux::SynchronizeEvent::new(
					get_time(t),
					input_linux::SynchronizeKind::Report,
					0,
				))
				.as_raw(),
			])
			.unwrap();

		std::thread::sleep(Duration::from_millis(5));
		t = t + Duration::from_millis(5);

		self.handle
			.write(&[*input_linux::InputEvent {
				time: get_time(t),
				kind: input_linux::EventKind::Key,
				code: code,
				value: 0,
			}
			.as_raw()])
			.unwrap();

		self.handle
			.write(&[
				*input_linux::InputEvent::from(input_linux::SynchronizeEvent::new(
					get_time(t),
					input_linux::SynchronizeKind::Report,
					0,
				))
				.as_raw(),
			])
			.unwrap();

		if k.caps {
			std::thread::sleep(Duration::from_millis(5));
			t = t + Duration::from_millis(5);

			self.handle
				.write(&[*input_linux::InputEvent::from(input_linux::KeyEvent::new(
					get_time(t),
					input_linux::Key::LeftShift,
					input_linux::KeyState::RELEASED,
				))
				.as_raw()])
				.unwrap();

			self.handle
				.write(&[
					*input_linux::InputEvent::from(input_linux::SynchronizeEvent::new(
						get_time(t),
						input_linux::SynchronizeKind::Report,
						0,
					))
					.as_raw(),
				])
				.unwrap();
		}
	}

	pub fn send_passthrough(&self, evs: &[input_linux::sys::input_event]) {
		self.handle.write(evs).unwrap();
	}
}
