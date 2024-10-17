use std::time::{Duration, Instant};
use std::io::{self, Write, Read};
use serialport;
use std::thread;
use tauri::{self, AppHandle, Emitter}; // Import Emitter along with AppHandle


const PACKET_SIZE: usize = 16;
const START_BYTE_1: u8 = 0xC7;
const START_BYTE_2: u8 = 0x7C;
const END_BYTE: u8 = 0x01;

#[derive(Clone, serde::Serialize)]
struct Payload {
    message: Vec<i16>,
}


pub fn receive_arduino_data(port_name: &str, app_handle: AppHandle) {
    match serialport::new(port_name, 230400)
        .timeout(Duration::from_secs(5))
        .open()
    {
        Ok(mut port) => {
            println!("Connected to device on port: {}", port_name);
            let start_command = b"START\r\n";
            if let Err(e) = port.write_all(start_command) {
                println!("Failed to send START command: {:?}", e);
            }

            thread::sleep(Duration::from_millis(4));
            let mut buffer: Vec<u8> = vec![0; 1024]; // Smaller buffer for quicker reads
            let mut accumulated_buffer: Vec<u8> = Vec::new();

            loop {
                match port.read(&mut buffer) {
                    Ok(size) => {
                        accumulated_buffer.extend_from_slice(&buffer[..size]);

                        // Process packets if we have enough bytes
                        while accumulated_buffer.len() >= PACKET_SIZE {
                            // Check for start bytes and end byte for each packet
                            if accumulated_buffer[0] == START_BYTE_1 && accumulated_buffer[1] == START_BYTE_2 {
                                if accumulated_buffer[PACKET_SIZE - 1] == END_BYTE {
                                    // Extract the packet
                                    let packet = accumulated_buffer.drain(..PACKET_SIZE).collect::<Vec<u8>>();
        
                                    // Extract counter byte and 6x 2-byte data values
                                    let counter = packet[2] as i16;
                                    let data: Vec<i16> = (0..6).map(|i| {
                                        let idx = 3 + (i * 2); // 4 is where the data starts
                                        let high = packet[idx] as i16;
                                        let low = packet[idx + 1] as i16;
                                        (high << 8) | low // Combine the two bytes into a 16-bit value
                                    }).collect();
                                    let mut data=data;
                                    data.push(counter);

                                    // println!("Received raw data: {:?}", data);
                                    // Emit the data to the frontend
                                    app_handle.emit("updateSerial", Payload { message: data}).unwrap();
                                    
                                } else {
                                    // Invalid end byte, skip the packet
                                    accumulated_buffer.drain(..1);
                                }
                            } else {
                                // Invalid start bytes, skip
                                accumulated_buffer.drain(..1);
                            }
                        }
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                        println!("Read timed out, retrying...");
                        continue;
                    }
                    Err(e) => {
                        println!("Error receiving data: {:?}", e);
                        break;
                    }
                }
            }
        }
        Err(e) => {
            println!("Failed to connect to device on {}: {}", port_name, e);
        }
    }
}

fn auto_detect_arduino() -> Option<String> {
    loop {
        let ports = serialport::available_ports().expect("No ports found!");

        for port_info in ports {
            let port_name = port_info.port_name;
            println!("Attempting to connect to port: {}", port_name);

            match serialport::new(&port_name, 230400)
                .timeout(Duration::from_secs(1))
                .open()
            {
                Ok(mut port) => {
                    let command = b"WHORU\n";

                    if let Err(e) = port.write_all(command) {
                        println!("Failed to write to port: {}. Error: {:?}", port_name, e);
                        continue;
                    }
                    port.flush().expect("Failed to flush port");
                    println!("Sending command...");

                    let mut buffer: Vec<u8> = vec![0; 1024];
                    let mut response = String::new();
                    let start_time = Instant::now();
                    let timeout = Duration::from_secs(2);

                    while start_time.elapsed() < timeout {
                        match port.read(&mut buffer) {
                            Ok(size) => {
                                if size > 0 {
                                    response.push_str(&String::from_utf8_lossy(&buffer[..size]));
                                    println!("Partial response: {}", response);

                                    if response.contains("UNO-R4") || response.contains("UNO-R3") {
                                        println!("Valid device found on port: {}", port_name);
                                        drop(port);
                                        return Some(port_name); // Return the found port name directly
                                    }
                                }
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => continue,
                            Err(e) => {
                                println!("Failed to read from port: {}. Error: {:?}", port_name, e);
                                break;
                            }
                        }
                    }

                    println!("Final response from port {}: {}", port_name, response);
                    drop(port);
                }
                Err(e) => {
                    println!("Failed to open port: {}. Error: {:?}", port_name, e);
                }
            }
        }

        println!("No valid device found, retrying in 5 seconds...");
        thread::sleep(Duration::from_secs(5)); // Wait before trying again
    }
}


pub fn monitor_device_connection(port_name: &str, app_handle: &AppHandle) {
    loop {
        match serialport::new(port_name, 230400)
            .timeout(Duration::from_secs(5))
            .open()
        {
            Ok(mut port) => {
                println!("Connected to device on port: {}", port_name);
                let start_command = b"START\r\n";
                if let Err(e) = port.write_all(start_command) {
                    println!("Failed to send START command: {:?}", e);
                }
                thread::sleep(Duration::from_millis(4));
                let mut buffer: Vec<u8> = vec![0; 1024];
                let mut accumulated_buffer: Vec<u8> = Vec::new();

                loop {
                    match port.read(&mut buffer) {
                        Ok(size) => {
                            accumulated_buffer.extend_from_slice(&buffer[..size]);

                            // Process packets if we have enough bytes
                            while accumulated_buffer.len() >= PACKET_SIZE {
                                if accumulated_buffer[0] == START_BYTE_1 && accumulated_buffer[1] == START_BYTE_2 {
                                    if accumulated_buffer[PACKET_SIZE - 1] == END_BYTE {
                                        // Extract the packet
                                        let packet = accumulated_buffer.drain(..PACKET_SIZE).collect::<Vec<u8>>();
        
                                        let counter = packet[2] as i16;
                                        let data: Vec<i16> = (0..6).map(|i| {
                                            let idx = 3 + (i * 2);
                                            let high = packet[idx] as i16;
                                            let low = packet[idx + 1] as i16;
                                            (high << 8) | low
                                        }).collect();
                                        let mut data = data;
                                        data.push(counter);
                                        app_handle.emit("updateSerial", Payload { message: data }).unwrap();
                                    } else {
                                        accumulated_buffer.drain(..1); // Invalid end byte, skip the packet
                                    }
                                } else {
                                    accumulated_buffer.drain(..1); // Invalid start bytes, skip
                                }
                            }
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                            println!("Read timed out, retrying...");
                            continue;
                        }
                        Err(e) => {
                            println!("Error receiving data: {:?}", e);
                            break; // Exit the loop on error
                        }
                    }
                }
            }
            Err(e) => {
                println!("Failed to connect to device on {}: {}", port_name, e);
                break; // Exit the loop on error
            }
        }

        println!("Device disconnected, checking for new devices...");
        thread::sleep(Duration::from_secs(5)); // Wait before trying again
        // Automatically re-check for devices
        if let Some(new_port_name) = auto_detect_arduino() {
            println!("Found new device: {}", new_port_name);
            // Here, you can restart the connection with the new port
            monitor_device_connection(&new_port_name, app_handle);
        }
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            println!("Starting auto-detection of Arduino...");
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                if let Some(port_name) = auto_detect_arduino() {
                    println!("Starting to receive data from: {}", port_name);
                    monitor_device_connection(&port_name, &app_handle); // Borrow app_handle
                } else {
                    println!("No valid device found.");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error while running Tauri application");
}
