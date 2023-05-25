use input_linux::Key;
use log;
extern crate hidapi;

//use objekt;
use chrono;
use lazy_static;
use timer;

use hidapi::DeviceInfo as DeviceInfoHID;
use hidapi::{HidApi, HidDevice};
use log::{error, info};

use std::borrow::Borrow;
use std::collections::HashMap;
use std::os::raw::{c_float, c_ushort};
use std::os::unix::prelude::OpenOptionsExt;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::{str, thread};
use timer::{Guard, Timer};
use wooting_analog_plugin_dev::wooting_analog_common::*;

extern crate env_logger;

const ANALOG_BUFFER_SIZE: usize = 48;
const ANALOG_MAX_SIZE: usize = 40;
const WOOTING_VID: u16 = 0x31e3;
const WOOTING_PID_MODE_MASK: u16 = 0xFFF0;

/// Struct holding the information we need to find the device and the analog interface
struct DeviceHardwareID {
	vid: u16,
	pid: u16,
	usage_page: u16,
	has_modes: bool,
}

#[derive(PartialEq, Debug)]
pub enum ReadErrors {
	DeviceDisconnected,
}

/// Trait which defines how the Plugin can communicate with a particular device
trait DeviceImplementation: Send + Sync {
	/// Gives the device hardware ID that can be used to obtain the analog interface for this device
	fn device_hardware_id(&self) -> DeviceHardwareID;

	/// Used to determine if the given `device` matches the hardware id given by `device_hardware_id`
	fn matches(&self, device: &DeviceInfoHID) -> bool {
		let hid = self.device_hardware_id();
		let pid = if hid.has_modes {
			device.product_id() & WOOTING_PID_MODE_MASK
		} else {
			device.product_id()
		};
		//Check if the pid & hid match
		pid.eq(&hid.pid)
			&& device.vendor_id().eq(&hid.vid)
			&& device.usage_page().eq(&hid.usage_page)
	}

	/// Convert the given raw `value` into the appropriate float value. The given value should be 0.0f-1.0f
	fn analog_value_to_float(&self, value: u8) -> f32 {
		(f32::from(value) / 255_f32).min(1.0)
	}

	/// Get the current set of pressed keys and their analog values from the given `device`. Using `buffer` to read into
	///
	/// `max_length` is not the max length of the report, it is the max number of key + analog value pairs to read
	fn get_analog_buffer(
		&self,
		device: &HidDevice,
		max_length: usize,
	) -> Result<Option<Vec<ReadKey>>, ReadErrors> {
		let mut buffer: [u8; ANALOG_BUFFER_SIZE] = [0; ANALOG_BUFFER_SIZE];
		let res = device.read_timeout(&mut buffer, -1);

		match res {
			Ok(len) => {
				// If the length is 0 then that means the read timed out, so we shouldn't use it to update values
				if len == 0 {
					return Ok(None).into();
				}
			}
			Err(e) => {
				error!("Failed to read buffer: {}", e);

				return Err(ReadErrors::DeviceDisconnected);
			}
		}
		//println!("{:?}", buffer);
		Ok(Some(
			buffer
				.chunks_exact(3) //Split it into groups of 3 as the analog report is in the format of 2 byte code + 1 byte analog value
				//.take(max_length) //Only take up to the max length of results. Doing this
				.filter(|&s| s[0] != 0 || s[1] != 0) //Get rid of entries where the code is 0
				.map(|s| {
					ReadKey {
						code: ((u16::from(s[0])) << 8) | u16::from(s[1]), // Convert the first 2 bytes into the u16 code
						value: self.analog_value_to_float(s[2]), //Convert the remaining byte into the float analog value
						ts: std::time::Instant::now(),
					}
				})
				.collect(),
		))
	}

	/// Get the unique device ID from the given `device_info`
	fn get_device_id(&self, device_info: &DeviceInfoHID) -> DeviceID {
		wooting_analog_plugin_dev::generate_device_id(
			device_info.serial_number().as_ref().unwrap_or(&"NO SERIAL"),
			device_info.vendor_id(),
			device_info.product_id(),
		)
	}
}

//clone_trait_object!(DeviceImplementation);

#[derive(Debug, Clone)]
struct WootingOne();

impl DeviceImplementation for WootingOne {
	fn device_hardware_id(&self) -> DeviceHardwareID {
		DeviceHardwareID {
			vid: 0x03EB,
			pid: 0xFF01,
			usage_page: 0xFF54,
			has_modes: false,
		}
	}

	fn analog_value_to_float(&self, value: u8) -> f32 {
		((f32::from(value) * 1.2) / 255_f32).min(1.0)
	}
}

#[derive(Debug, Clone)]
struct WootingTwo();

impl DeviceImplementation for WootingTwo {
	fn device_hardware_id(&self) -> DeviceHardwareID {
		DeviceHardwareID {
			vid: 0x03EB,
			pid: 0xFF02,
			usage_page: 0xFF54,
			has_modes: false,
		}
	}

	fn analog_value_to_float(&self, value: u8) -> f32 {
		((f32::from(value) * 1.2) / 255_f32).min(1.0)
	}
}

#[derive(Debug, Clone)]
struct WootingOneV2();

impl DeviceImplementation for WootingOneV2 {
	fn device_hardware_id(&self) -> DeviceHardwareID {
		DeviceHardwareID {
			vid: WOOTING_VID,
			pid: 0x1100,
			usage_page: 0xFF54,
			has_modes: true,
		}
	}
}

#[derive(Debug, Clone)]
struct WootingTwoV2();

impl DeviceImplementation for WootingTwoV2 {
	fn device_hardware_id(&self) -> DeviceHardwareID {
		DeviceHardwareID {
			vid: WOOTING_VID,
			pid: 0x1200,
			usage_page: 0xFF54,
			has_modes: true,
		}
	}
}
#[derive(Debug, Clone)]
struct WootingLekker();

impl DeviceImplementation for WootingLekker {
	fn device_hardware_id(&self) -> DeviceHardwareID {
		DeviceHardwareID {
			vid: WOOTING_VID,
			pid: 0x1210,
			usage_page: 0xFF54,
			has_modes: true,
		}
	}
}

#[derive(Debug, Clone)]
struct WootingTwoHE();

impl DeviceImplementation for WootingTwoHE {
	fn device_hardware_id(&self) -> DeviceHardwareID {
		DeviceHardwareID {
			vid: WOOTING_VID,
			pid: 0x1220,
			usage_page: 0xFF54,
			has_modes: true,
		}
	}
}

#[derive(Debug, Clone)]
struct WootingTwoHEARM();

impl DeviceImplementation for WootingTwoHEARM {
	fn device_hardware_id(&self) -> DeviceHardwareID {
		DeviceHardwareID {
			vid: WOOTING_VID,
			pid: 0x1230,
			usage_page: 0xFF54,
			has_modes: true,
		}
	}
}

#[derive(Debug, Clone)]
struct Wooting60HE();

impl DeviceImplementation for Wooting60HE {
	fn device_hardware_id(&self) -> DeviceHardwareID {
		DeviceHardwareID {
			vid: WOOTING_VID,
			pid: 0x1300,
			usage_page: 0xFF54,
			has_modes: true,
		}
	}
}

#[derive(Debug, Clone)]
struct Wooting60HEARM();

// mess
pub struct ReadKey {
	pub code: u16,
	pub value: f32,
	pub ts: std::time::Instant,
}

const READ_CHANNEL_BUF_SIZE: usize = 4;

impl DeviceImplementation for Wooting60HEARM {
	fn device_hardware_id(&self) -> DeviceHardwareID {
		DeviceHardwareID {
			vid: WOOTING_VID,
			pid: 0x1310,
			usage_page: 0xFF54,
			has_modes: true,
		}
	}
}
/// A fully contained device which uses `device_impl` to interface with the `device`
struct Device {
	pub device_info: DeviceInfo,
	//buffer: Arc<Mutex<HashMap<c_ushort, c_float>>>,
	//sender: SyncSender<Vec<ReadKey>>,
	connected: Arc<AtomicBool>,
	pressed_keys: Vec<u16>,
	worker: Option<JoinHandle<i32>>,
}
unsafe impl Send for Device {}

impl Device {
	fn new(
		device_info: &DeviceInfoHID,
		device: HidDevice,
		device_impl: &'static Box<dyn DeviceImplementation>,
		sender: SyncSender<Vec<ReadKey>>,
	) -> (DeviceID, Self) {
		let id_hash = device_impl.get_device_id(device_info);

		// let buffer: Arc<Mutex<HashMap<c_ushort, c_float>>> =
		//     Arc::new(Mutex::new(Default::default()));
		let connected = Arc::new(AtomicBool::new(true));

		device.set_blocking_mode(true).unwrap();

		let worker = {
			//let t_buffer = Arc::clone(&buffer);
			let t_connected = Arc::clone(&connected);

			thread::spawn(move || loop {
				if !t_connected.load(Ordering::Relaxed) {
					return 0;
				}

				match device_impl
					.get_analog_buffer(&device, ANALOG_MAX_SIZE)
					.into()
				{
					Ok(Some(data)) => {
						if let Err(e) = sender.send(data) {
							error!("Sending failed, disconnected? {e:?}");
							panic!("bang")
						}
					}
					Ok(None) => {}
					Err(e) => {
						if e != ReadErrors::DeviceDisconnected {
							error!("Read failed from device that isn't DeviceDisconnected, we got {:?}. Disconnecting device...", e);
						}
						t_connected.store(false, Ordering::Relaxed);
						return 0;
					}
				}
			})
		};

		(
			id_hash,
			Device {
				device_info: DeviceInfo::new_with_id(
					device_info.vendor_id(),
					device_info.product_id(),
					device_info
						.manufacturer_string()
						.unwrap_or("ERR COULD NOT BE FOUND")
						.to_string(),
					device_info
						.product_string()
						.unwrap_or("ERR COULD NOT BE FOUND")
						.to_string(),
					id_hash,
					DeviceType::Keyboard,
				),
				connected,
				//buffer,
				//sender: sender,
				pressed_keys: vec![],
				worker: Some(worker),
			},
		)
	}

	// fn read_analog(&mut self, code: u16) -> SDKResult<c_float> {
	//     (*self.buffer.lock().unwrap().get(&code).unwrap_or(&0.0)).into()
	// }

	// fn read_full_buffer(&mut self, _max_length: usize) -> SDKResult<HashMap<c_ushort, c_float>> {
	//     let mut buffer = self.buffer.lock().unwrap().clone();
	//     //Collect the new pressed keys
	//     let new_pressed_keys: Vec<u16> = buffer.keys().map(|x| *x).collect();

	//     //Put the old pressed keys into the buffer
	//     for key in self.pressed_keys.drain(..) {
	//         if !buffer.contains_key(&key) {
	//             buffer.insert(key, 0.0);
	//         }
	//     }

	//     //Store the newPressedKeys for the next call
	//     self.pressed_keys = new_pressed_keys;

	//     Ok(buffer).into()
	// }
}

impl Drop for Device {
	fn drop(&mut self) {
		//self.device_info.clone().drop();
		//Set the device to connected so the thread will stop if it hasn't already
		self.connected.store(false, Ordering::Relaxed);
		if let Some(worker) = self.worker.take() {
			worker
				.join()
				.expect("Couldn't join on the associated thread");
		}
	}
}

pub struct WootingPlugin {
	initialised: bool,
	device_event_cb: Arc<Mutex<Option<Box<dyn Fn(DeviceEventType, &DeviceInfo) + Send>>>>,
	devices: Arc<Mutex<HashMap<DeviceID, (Device, EvdevDevice)>>>,
	timer: Timer,
	worker_guard: Option<Guard>,
}

lazy_static::lazy_static! {
static ref DEVICE_IMPLS: Vec<Box<dyn DeviceImplementation>> = vec![
	Box::new(WootingOne()),
	Box::new(WootingTwo()),
	Box::new(WootingOneV2()),
	Box::new(WootingTwoV2()),
	Box::new(WootingLekker()),
	Box::new(WootingTwoHE()),
	Box::new(WootingTwoHEARM()),
	Box::new(Wooting60HE()),
	Box::new(Wooting60HEARM()),
];
}

const PLUGIN_NAME: &str = "Wooting Official Plugin";
impl WootingPlugin {
	pub fn new() -> Self {
		WootingPlugin {
			initialised: false,
			device_event_cb: Arc::new(Mutex::new(None)),
			devices: Arc::new(Mutex::new(Default::default())),
			timer: timer::Timer::new(),
			worker_guard: None,
		}
	}

	pub fn initialise(
		&mut self,
		callback: Box<dyn Fn(DeviceEventType, &DeviceInfo) + Send>,
	) -> SDKResult<(u32, std::sync::mpsc::Receiver<Vec<ReadKey>>)> {
		if let Err(e) = env_logger::try_init() {
			log::warn!("Unable to initialize Env Logger: {}", e);
		}

		let ret = self.init_worker();
		self.device_event_cb.lock().unwrap().replace(callback);
		self.initialised = ret.is_ok();
		ret
	}

	fn init_worker(&mut self) -> SDKResult<(u32, std::sync::mpsc::Receiver<Vec<ReadKey>>)> {
		let (tx, rx) = std::sync::mpsc::sync_channel(READ_CHANNEL_BUF_SIZE);
		let init_device_closure = |hid: &HidApi,
		                           devices: &Arc<
			Mutex<HashMap<DeviceID, (Device, EvdevDevice)>>,
		>,
		                           device_event_cb: &Arc<
			Mutex<Option<Box<dyn Fn(DeviceEventType, &DeviceInfo) + Send>>>,
		>,
		                           tx: SyncSender<Vec<ReadKey>>| {
			let device_infos: Vec<&DeviceInfoHID> = hid.device_list().collect();

			for device_info in device_infos.iter() {
				let m = device_info.manufacturer_string().unwrap_or_default();
				let pr = device_info.product_string().unwrap_or_default();
				let p = device_info.path().to_string_lossy();
				let u = device_info.usage();
				let up = device_info.usage_page();
				//info!("device_info: {m}\n{pr}\n{p}\n{u}\n{up} {device_info:?}");
				for device_impl in DEVICE_IMPLS.iter() {
					if device_impl.matches(device_info)
						&& !devices
							.lock()
							.unwrap()
							.contains_key(&device_impl.get_device_id(device_info))
					{
						// info!("Found device impl match: {:?}", device_info);
						let evdev_path = find_evdev(device_info.product_id());

						let dev = match device_info.open_device(&hid) {
							Ok(dev) => dev,

							Err(e) => {
								error!("Error opening HID Device: {}", e);
								continue;
								//return WootingAnalogResult::Failure.into();
							}
						};

						let (id, device) = Device::new(device_info, dev, device_impl, tx.clone());
						let ev = EvdevDevice::new(&evdev_path);

						{
							devices.lock().unwrap().insert(id, (device, ev));
						}

						info!(
							"Found and opened the {:?} successfully!",
							device_info.product_string()
						);

						device_event_cb.lock().unwrap().as_ref().and_then(|cb| {
							cb(
								DeviceEventType::Connected,
								devices
									.lock()
									.unwrap()
									.get(&id)
									.unwrap()
									.0
									.device_info
									.borrow(),
							);
							Some(0)
						});
					}
				}
			}
		};

		let mut hid = match HidApi::new() {
			Ok(mut api) => {
				//An attempt at trying to ensure that all the devices have been found in the initialisation of the plugins
				if let Err(e) = api.refresh_devices() {
					error!("We got error while refreshing devices. Err: {}", e);
				}
				api
			}
			Err(e) => {
				error!("Error obtaining HIDAPI: {}", e);
				return Err(WootingAnalogResult::Failure).into();
			}
		};

		//We wanna call it in this thread first so we can get hold of any connected devices now so we can return an accurate result for initialise
		init_device_closure(&hid, &self.devices, &self.device_event_cb, tx.clone());

		self.worker_guard = Some({
			let t_devices = Arc::clone(&self.devices);
			let t_device_event_cb = Arc::clone(&self.device_event_cb);
			self.timer
				.schedule_repeating(chrono::Duration::milliseconds(500), move || {
					//Check if any of the devices have disconnected and get rid of them if they have
					{
						let mut disconnected: Vec<u64> = vec![];
						for (&id, (device, ev)) in t_devices.lock().unwrap().iter() {
							if !device.connected.load(Ordering::Relaxed) {
								disconnected.push(id);
							}
						}

						for id in disconnected.iter() {
							let (device, ev) = t_devices.lock().unwrap().remove(id).unwrap();
							t_device_event_cb.lock().unwrap().as_ref().and_then(|cb| {
								cb(DeviceEventType::Disconnected, &device.device_info);
								Some(0)
							});
						}
					}

					if let Err(e) = hid.refresh_devices() {
						error!("We got error while refreshing devices. Err: {}", e);
					}
					init_device_closure(&hid, &t_devices, &t_device_event_cb, tx.clone());
				})
		});
		log::debug!("Started timer");
		Ok((self.devices.lock().unwrap().len() as u32, rx)).into()
	}
}

impl WootingPlugin {
	fn name(&mut self) -> SDKResult<&'static str> {
		Ok(PLUGIN_NAME).into()
	}

	fn is_initialised(&mut self) -> bool {
		self.initialised
	}

	fn unload(&mut self) {
		self.devices.lock().unwrap().drain();
		drop(self.worker_guard.take());
		self.initialised = false;

		info!("{} unloaded", PLUGIN_NAME);
	}

	// fn read_analog(&mut self, code: u16, device_id: DeviceID) -> SDKResult<f32> {
	//     if !self.initialised {
	//         return Err(WootingAnalogResult::UnInitialized).into();
	//     }

	//     if self.devices.lock().unwrap().is_empty() {
	//         return Err(WootingAnalogResult::NoDevices).into();
	//     }

	//     //If the Device ID is 0 we want to go through all the connected devices
	//     //and combine the analog values
	//     if device_id == 0 {
	//         let mut analog: f32 = -1.0;
	//         let mut error: WootingAnalogResult = WootingAnalogResult::Ok;
	//         for (_id, device) in self.devices.lock().unwrap().iter_mut() {
	//             match device.read_analog(code).into() {
	//                 Ok(val) => {
	//                     analog = analog.max(val);
	//                 }
	//                 Err(e) => {
	//                     error = e;
	//                 }
	//             }
	//         }

	//         if analog < 0.0 {
	//             Err(error).into()
	//         } else {
	//             analog.into()
	//         }
	//     } else
	//     //If the device id is not 0, we try and find a connected device with that ID and read from it
	//     {
	//         match self.devices.lock().unwrap().get_mut(&device_id) {
	//             Some(device) => match device.read_analog(code).into() {
	//                 Ok(val) => val.into(),
	//                 Err(e) => Err(e).into(),
	//             },
	//             None => Err(WootingAnalogResult::NoDevices).into(),
	//         }
	//     }
	// }

	// fn read_full_buffer(
	//     &mut self,
	//     max_length: usize,
	//     device_id: DeviceID,
	// ) -> SDKResult<HashMap<c_ushort, c_float>> {
	//     if !self.initialised {
	//         return Err(WootingAnalogResult::UnInitialized).into();
	//     }

	//     if self.devices.lock().unwrap().is_empty() {
	//         return Err(WootingAnalogResult::NoDevices).into();
	//     }

	//     //If the Device ID is 0 we want to go through all the connected devices
	//     //and combine the analog values
	//     if device_id == 0 {
	//         let mut analog: HashMap<c_ushort, c_float> = HashMap::new();
	//         let mut any_read = false;
	//         let mut error: WootingAnalogResult = WootingAnalogResult::Ok;
	//         for (_id, device) in self.devices.lock().unwrap().iter_mut() {
	//             match device.read_full_buffer(max_length).into() {
	//                 Ok(val) => {
	//                     any_read = true;
	//                     analog.extend(val);
	//                 }
	//                 Err(e) => {
	//                     error = e;
	//                 }
	//             }
	//         }

	//         if !any_read {
	//             Err(error).into()
	//         } else {
	//             Ok(analog).into()
	//         }
	//     } else
	//     //If the device id is not 0, we try and find a connected device with that ID and read from it
	//     {
	//         match self.devices.lock().unwrap().get_mut(&device_id) {
	//             Some(device) => match device.read_full_buffer(max_length).into() {
	//                 Ok(val) => Ok(val).into(),
	//                 Err(e) => Err(e).into(),
	//             },
	//             None => Err(WootingAnalogResult::NoDevices).into(),
	//         }
	//     }
	// }

	fn device_info(&mut self) -> SDKResult<Vec<DeviceInfo>> {
		if !self.initialised {
			return Err(WootingAnalogResult::UnInitialized).into();
		}

		let mut devices = vec![];
		for (_id, (device, _)) in self.devices.lock().unwrap().iter() {
			devices.push(device.device_info.clone());
		}

		Ok(devices).into()
	}
}

fn find_evdev(product_id: u16) -> std::path::PathBuf {
	use input_linux;
	use std::os::unix::ffi::OsStringExt;
	let des = std::fs::read_dir("/dev/input").unwrap();
	let evdev_direntries = des.into_iter().filter_map(|f| {
		let de = f.unwrap();
		if de.file_name().to_string_lossy().starts_with("event") {
			Some(de.path())
		} else {
			None
		}
	});
	let mut found_keyboards = evdev_direntries
		.filter_map(|path| {
			let fd = std::fs::OpenOptions::new().read(true).open(&path).unwrap();
			let ev = input_linux::EvdevHandle::new(fd);
			let id = ev.device_id().unwrap();
			if id.vendor != WOOTING_VID && id.product != product_id {
				return None;
			}
			let e = ev.key_bits().unwrap();
			if !e
				.into_iter()
				.collect::<Vec<_>>()
				.contains(&input_linux::Key::A)
			{
				return None;
			}
			Some((path, ev))
		})
		.collect::<Vec<_>>();

	if found_keyboards.len() != 1 {
		panic!("found 0 or multiple evdev candidates for keyboard!")
	}
	let (path, kb) = found_keyboards.remove(0);

	let id = kb.device_id().unwrap();
	let phys = &std::ffi::OsString::from_vec(kb.device_name().unwrap());
	let name = &std::ffi::OsString::from_vec(kb.physical_location().unwrap());
	log::info!("{phys:?} {name:? }{id:#?}");
	drop(kb);

	return path;
}

struct EvdevDevice {
	h: input_linux::EvdevHandle<std::fs::File>,
}
impl EvdevDevice {
	fn new(path: &PathBuf) -> Self {
		let fd = std::fs::OpenOptions::new()
			.read(true)
			.open(path)
			.expect(&format!("couldn\'t open {path:?}"));

		let h = input_linux::EvdevHandle::new(fd);
		h.grab(true).unwrap();

		EvdevDevice { h }
	}
}

//declare_plugin!(WootingPlugin, WootingPlugin::new);
