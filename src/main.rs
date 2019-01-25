

// use env_logger::init;

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
use structopt::StructOpt;

/// MWatch Protocol Spoofer
#[derive(StructOpt, Debug)]
#[structopt(name = "MWatch Protocol Spoofer")]
struct Opt {
    /// Activate debug mode
    #[structopt(short = "d", long = "debug")]
    debug: bool,

    /// binary to flash to the watch
    #[structopt(short = "b", long = "binary", parse(from_os_str), default_value = "")]
    output: PathBuf,

    /// Message to send, defaults to 'Hello MWatch'
    #[structopt(short = "m", long = "message", default_value = "Hello MWatch")]
    message: String,
}

#[derive(Clone)]
pub struct Handle<'a> {
    session: &'a Session,
    device: Device<'a>,
    // service: Service<'a>,
    characteristic: Characteristic<'a>,
}

fn main() -> Result<(), Box<Error>> {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    let bt_session = &Session::create_session(None)?;
    match find_device("MWatch", &bt_session) {
        Ok(mut handle) => {
            if opt.output.to_str().unwrap() == "" {
                spoof_msg(&opt, &mut handle).unwrap();
            } else {
                send_binary(&opt, &mut handle).unwrap();
            }
            // always disconnect at the end
            handle.device.disconnect()?
        },
        Err(e) => println!("{:?}", e),
    }
    Ok(())
}

fn spoof_msg(opt: &Opt, handle : &mut Handle) -> Result<(), Box<Error>> {
    let mut data = vec![2, b'N', 31]; // N for notification
    data.append(&mut opt.message.clone().into_bytes());
    data.push(3u8); // ETX
    for chunk in data.chunks(10) {
        handle.characteristic.write_value(chunk.to_vec(), None).unwrap();
    }
    Ok(())
}

fn send_binary(opt: &Opt, handle : &mut Handle) -> Result<(), Box<Error>> {
    unimplemented!()
}

fn find_device<'a>(name: &'a str, bt_session: &'a Session) -> Result<Handle<'a>, Box<Error>> {
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
    println!("{} device(s) found", devices.len());
    let mut device: Device = Device::new(bt_session, "".to_string());
    'device_loop: for d in devices {
        device = Device::new(bt_session, d.clone());
        println!("{} {:?}", device.get_id(), device.get_alias());
        let uuids = device.get_uuids()?;
        // println!("{:?}", uuids);
        'uuid_loop: for uuid in uuids {
            if uuid == SERVICE && name == device.get_alias().unwrap().as_str() {
                println!("{:?} has a service!", device.get_alias().unwrap());
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
