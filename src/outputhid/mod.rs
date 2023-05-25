use std::time::Duration;

use input_linux::{uinput, InputEvent};

use crate::watcher::KeyEvent;
pub struct OutputHid {
	handle: input_linux::uinput::UInputHandle<std::fs::File>,
}

impl OutputHid {
	pub fn new() -> Self {
		use std::os::unix::fs::OpenOptionsExt;

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

		return OutputHid { handle };
	}
	pub fn send_key(&mut self, k: &KeyEvent) {
		let code = k.code;
		let velocity = k.velocity;
		let time = input_linux::EventTime::new(0, 0);
		let Ok(key) = input_linux::Key::from_code(code) else {
			log::warn!("ignoring bad code {code}");
			return;
		};
		let code = Into::<u16>::into(key);

		log::info!("diff for {key:?}/{code:?} is {velocity}");

		if k.caps {
			self.handle
				.write(&[*input_linux::InputEvent::from(input_linux::KeyEvent::new(
					time,
					input_linux::Key::LeftShift,
					input_linux::KeyState::PRESSED,
				))
				.as_raw()])
				.unwrap();

			self.handle
				.write(&[
					*input_linux::InputEvent::from(input_linux::SynchronizeEvent::new(
						time,
						input_linux::SynchronizeKind::Report,
						0,
					))
					.as_raw(),
				])
				.unwrap();

			std::thread::sleep(Duration::from_millis(15));
		}

		self.handle
			.write(&[*input_linux::InputEvent {
				time: time,
				kind: input_linux::EventKind::Key,
				code: code,
				value: 1,
			}
			.as_raw()])
			.unwrap();

		self.handle
			.write(&[
				*input_linux::InputEvent::from(input_linux::SynchronizeEvent::new(
					time,
					input_linux::SynchronizeKind::Report,
					0,
				))
				.as_raw(),
			])
			.unwrap();

		std::thread::sleep(Duration::from_millis(15));

		self.handle
			.write(&[*input_linux::InputEvent {
				time: time,
				kind: input_linux::EventKind::Key,
				code: code,
				value: 0,
			}
			.as_raw()])
			.unwrap();

		self.handle
			.write(&[
				*input_linux::InputEvent::from(input_linux::SynchronizeEvent::new(
					time,
					input_linux::SynchronizeKind::Report,
					0,
				))
				.as_raw(),
			])
			.unwrap();
		if k.caps {
			std::thread::sleep(Duration::from_millis(15));

			self.handle
				.write(&[*input_linux::InputEvent::from(input_linux::KeyEvent::new(
					time,
					input_linux::Key::LeftShift,
					input_linux::KeyState::RELEASED,
				))
				.as_raw()])
				.unwrap();

			self.handle
				.write(&[
					*input_linux::InputEvent::from(input_linux::SynchronizeEvent::new(
						time,
						input_linux::SynchronizeKind::Report,
						0,
					))
					.as_raw(),
				])
				.unwrap();
		}
	}
}
