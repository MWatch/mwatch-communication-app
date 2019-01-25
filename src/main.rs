

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
use blurz::bluetooth_gatt_descriptor::BluetoothGATTDescriptor as Descriptor;
use blurz::bluetooth_gatt_service::BluetoothGATTService as Service;
use blurz::bluetooth_session::BluetoothSession as Session;

fn find_device(name: &str) -> Result<(), Box<Error>> {
    let bt_session = &Session::create_session(None)?;
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
                    thread::sleep(Duration::from_millis(5000));
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
    spoof(device, &bt_session)?;
    // let services = device.get_gatt_services()?;
    // for service in services {
    //     let s = Service::new(bt_session, service.clone());
    //     println!("{:?}", s);
    //     let characteristics = s.get_gatt_characteristics()?;
    //     for characteristic in characteristics {
    //         let c = Characteristic::new(bt_session, characteristic.clone());
    //         println!("{:?}", c);
    //         println!("Value: {:?}", c.read_value(None));
    //         let descriptors = c.get_gatt_descriptors()?;
    //         for descriptor in descriptors {
    //             let d = Descriptor::new(bt_session, descriptor.clone());
    //             println!("{:?}", d);
    //             println!("Value: {:?}", d.read_value(None));
    //         }
    //     }
    // }
    // device.disconnect()?;
    Ok(())
}

fn spoof(device: Device, bt_session: &blurz::BluetoothSession) -> Result<(), Box<Error>> {
    let services = device.get_gatt_services()?;
    let mut write_characteristic: Option<Characteristic> = None;
    for service in services {
        let s = Service::new(bt_session, service.clone());
        println!("{:?}", s);
        let characteristics = s.get_gatt_characteristics()?;
        for characteristic in characteristics {
            let c = Characteristic::new(bt_session, characteristic.clone());
            if c.get_uuid().unwrap().as_str() == CHARACTERISTIC {
                write_characteristic = Some(c);
                break;
            }
            println!("{:?}", c);
            println!("Value: {:?}", c.read_value(None));
            let descriptors = c.get_gatt_descriptors()?;
            for descriptor in descriptors {
                let d = Descriptor::new(bt_session, descriptor.clone());
                println!("{:?}", d);
                println!("Value: {:?}", d.read_value(None));
            }
        }
    }
    let chara = write_characteristic.unwrap();
    let mut data = vec![2, b'N', 31];
    for byte in "Hello From Rust!".bytes() {
        data.push(byte);
    }
    data.push(3u8); // ETX
    chara.write_value(data, None).unwrap();
    Ok(())
}

fn main() {
    match find_device("MWatch") {
        Ok(_) => (),
        Err(e) => println!("{:?}", e),
    }
}
