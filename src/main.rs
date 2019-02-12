

pub const SERVICE: &'static str = "0000ffe0-0000-1000-8000-00805f9b34fb";
pub const CHARACTERISTIC: &'static str = "0000ffe1-0000-1000-8000-00805f9b34fb";

extern crate blurz;

use std::error::Error;
use std::thread;
use std::time::Duration;

use blurz::bluetooth_adapter::BluetoothAdapter as Adapter;
use blurz::bluetooth_device::BluetoothDevice as Device;
use blurz::bluetooth_discovery_session::BluetoothDiscoverySession as DiscoverySession;
use blurz::bluetooth_gatt_characteristic::BluetoothGATTCharacteristic as Characteristic;
// use blurz::bluetooth_gatt_descriptor::BluetoothGATTDescriptor as Descriptor;
use blurz::bluetooth_gatt_service::BluetoothGATTService as Service;
use blurz::bluetooth_session::BluetoothSession as Session;

use std::path::PathBuf;
use std::io::prelude::*;
use std::fs::File;
use structopt::StructOpt;

use simple_hex::bytes_to_hex;

use crc::crc32::checksum_ieee;

/// MWatch Protocol Spoofer
#[derive(StructOpt, Debug)]
#[structopt(name = "MWatch Protocol Spoofer")]
struct Opt {
    /// Activate debug mode
    #[structopt(short = "v", long = "verbose")]
    debug: bool,

    /// binary to flash to the watch
    #[structopt(short = "b", long = "binary", parse(from_os_str), default_value = "")]
    binary: PathBuf,

    /// Invoke a syscall on the mwatch
    #[structopt(short = "s", long = "syscall", default_value = "")]
    syscall: String,

    /// Message to send
    #[structopt(short = "m", long = "message", default_value = "")]
    message: String,

    /// Delay between chunk transmission
    #[structopt(short = "d", long = "delay", default_value = "25")]
    delay: u64,
}

#[derive(Clone, Debug)]
pub struct Handle<'a> {
    session: &'a Session,
    device: Device<'a>,
    // service: Service<'a>,
    characteristic: Characteristic<'a>,
}

fn main() -> Result<(), Box<Error>> {
    let opt = Opt::from_args();
    if opt.debug {
        println!("{:?}", opt);
    }
    let bt_session = &Session::create_session(None)?;
    match find_device(&opt, "MWatch", &bt_session) {
        Ok(mut handle) => {
            if !opt.message.is_empty() {
                spoof_msg(&opt, &mut handle).unwrap();
            } else if !opt.binary.clone().into_os_string().is_empty() {
                send_binary(&opt, &mut handle).unwrap();
            } else if !opt.syscall.is_empty() {
                send_syscall(&opt, &mut handle).unwrap();
            } else {
                panic!("Invalid args")
            }
            println!("finished transmission.");
            // always disconnect at the end
            handle.device.disconnect()?;
            println!("Disconnected.");
        },
        Err(e) => println!("{:?}", e),
    }
    Ok(())
}

fn spoof_msg(opt: &Opt, handle : &mut Handle) -> Result<(), Box<Error>> {
    let mut data = vec![2, b'N', 31]; // N for notification
    data.append(&mut opt.message.clone().into_bytes());
    data.push(3u8); // ETX
    send(handle, data, opt.delay)
}

fn send_syscall(opt: &Opt, handle : &mut Handle) -> Result<(), Box<Error>> {
    let mut data = vec![2, b'N', 31]; // N for notification
    let mut syscall = opt.syscall.clone().into_bytes();
    if opt.debug {
        println!("Sending syscall: {:?}", syscall);
    }
    data.append(&mut syscall);
    data.push(3u8); // ETX
    send(handle, data, opt.delay)
}

/// Basic structure STX -> Type e.g A for Application -> (DELIM -> DATA)* -> ETX
/// * inidicates an number of delimiters followed by data
fn send_binary(opt: &Opt, handle: &mut Handle) -> Result<(), Box<Error>> {
    let mut prepend = vec![2, b'A', 31]; // A for application
    let mut buffer = Vec::new();
    let mut file = File::open(&opt.binary)?;
    file.read_to_end(&mut buffer)?;

    let digest = checksum_ieee(&buffer);
    let bytes: [u8; 4] = transform_u32_to_array_of_u8(digest);
    let mut digest_hex_bytes = vec![0u8; bytes.len() * 2];
    bytes_to_hex(&bytes, &mut digest_hex_bytes).unwrap();
    prepend.append(&mut digest_hex_bytes);
    prepend.push(31); //DELIM

    let total = (buffer.len() * 2) + prepend.len();
    let mut hexed = vec![0u8; total];

    for i in 0..prepend.len() {
        hexed[i] = prepend[i]
    }
    
    bytes_to_hex(&buffer, &mut hexed[prepend.len()..]).unwrap();
    hexed.push(3u8); // ETX
    if opt.debug {
        println!("Binary size {}", buffer.len());
        println!("Digest of binary: 0x{:08X}, as u32 {}", digest, digest);
        println!("HEXED[{}]: {:?}", hexed.len(), hexed);
    }
    send(handle, hexed, opt.delay)
}

fn transform_u32_to_array_of_u8(x:u32) -> [u8;4] {
    let b1 : u8 = ((x >> 24) & 0xff) as u8;
    let b2 : u8 = ((x >> 16) & 0xff) as u8;
    let b3 : u8 = ((x >> 8) & 0xff) as u8;
    let b4 : u8 = (x & 0xff) as u8;
    return [b1, b2, b3, b4]
}

fn send(handle: &mut Handle, data: Vec<u8>, delay: u64) -> Result<(), Box<Error>> {
    for chunk in data.chunks(16) {
        handle.characteristic.write_value(chunk.to_vec(), None)?;
        thread::sleep(Duration::from_millis(delay));
    }
    Ok(())
}

fn find_device<'a>(opt: &Opt, name: &'a str, bt_session: &'a Session) -> Result<Handle<'a>, Box<Error>> {
    let adapter: Adapter = Adapter::init(bt_session)?;
    let session = DiscoverySession::create_session(
        &bt_session,
        adapter.get_id()
    )?;
    session.start_discovery()?;
    //let mut devices = vec!();
    for _ in 0..5 {
        let devices = adapter.get_device_list()?;
        if !devices.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(1000));
    }
    session.stop_discovery()?;
    let devices = adapter.get_device_list()?;
    if devices.is_empty() {
        return Err(Box::from("No device found"));
    }
    if opt.debug {
        println!("{} device(s) found", devices.len());
    }
    let mut device: Device = Device::new(bt_session, "".to_string());
    'device_loop: for d in devices {
        device = Device::new(bt_session, d.clone());
        let uuids = device.get_uuids()?;
        if opt.debug {
            println!("{} {:?}", device.get_id(), device.get_alias());
            println!("{:?}", uuids);
        }
        'uuid_loop: for uuid in uuids {
            if uuid == SERVICE && name == device.get_alias().unwrap().as_str() {
                println!("Device {:?} has the correct service!", device.get_alias().unwrap());
                println!("connect device...");
                device.connect(10000).ok();
                if device.is_connected().unwrap() {
                    println!("checking gatt...");
                    // We need to wait a bit after calling connect to safely
                    // get the gatt services
                    thread::sleep(Duration::from_millis(250));
                    match device.get_gatt_services() {
                        Ok(_) => break 'device_loop,
                        Err(e) => println!("{:?}", e),
                    }
                } else {
                    println!("could not connect");
                }
            }
        }
        println!("");
    }
    adapter.stop_discovery().ok();
    if !device.is_connected().unwrap() {
        return Err(Box::from("No connectable device found"));
    }
    let services = device.get_gatt_services()?;
    let mut write_characteristic = Characteristic::new(bt_session, "".to_string());
    for service in services {
        let s = Service::new(bt_session, service.clone());
        // println!("{:?}", s);
        let characteristics = s.get_gatt_characteristics()?;
        for characteristic in characteristics {
            let c = Characteristic::new(bt_session, characteristic.clone());
            if c.get_uuid().unwrap().as_str() == CHARACTERISTIC {
                write_characteristic = c;
                break;
            }
        }
    }
    // device.disconnect()?;
    Ok(Handle {
        session: bt_session,
        device: device,
        characteristic: write_characteristic
    })
}
