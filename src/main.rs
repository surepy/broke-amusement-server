use std::ffi::CStr;
use std::ffi::OsString;
use std::io::Read;
use std::net::TcpListener;
use std::os::windows::ffi::OsStringExt;
use std::thread;
use std::time::Duration;
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::tlhelp32::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use winapi::um::winnt::PROCESS_QUERY_INFORMATION;

mod game;
use crate::game::{GameInstance, SegaToolsInstance, SpiceGameInstance};
mod card;

fn create_game_instance(
    process_name: &str,
    entry: &PROCESSENTRY32W,
) -> Option<Box<dyn GameInstance>> {
    match process_name {
        "spice.exe" | "spice64.exe" => {
            let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, 0, entry.th32ProcessID) };
            if handle.is_null() {
                return None;
            }
            Some(Box::new(SpiceGameInstance::new(handle)))
        }
        // I dont know if amdaemon doesnt run on other games
        // I know that it runs on chunithm and ongeki /shrug
        "amdaemon.exe" => {
            let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, 0, entry.th32ProcessID) };
            if handle.is_null() {
                return None;
            }
            Some(Box::new(SegaToolsInstance::new(handle)))
        }
        _ => None,
    }
}

// loops until it can find a game handle under
// "spice64.exe" or "amdaemon.exe"
fn find_game_instance() -> Box<dyn GameInstance> {
    loop {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);

            // this realistically shouldn't happen, but just in case.
            if snapshot == winapi::um::handleapi::INVALID_HANDLE_VALUE {
                eprintln!("Warn: 'snapshot == winapi::um::handleapi::INVALID_HANDLE_VALUE'!");
                thread::sleep(Duration::from_millis(1000));
                continue;
            }

            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let process_name_wide = &entry.szExeFile[..entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len())];
                    let process_name = OsString::from_wide(process_name_wide)
                        .to_string_lossy()
                        .to_lowercase();

                    if let Some(instance) = create_game_instance(&process_name, &entry) {
                        CloseHandle(snapshot);
                        println!(
                            "Found Game Instance! | name = {} PID = {}",
                            process_name, entry.th32ProcessID
                        );
                        return instance;
                    }

                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }

            CloseHandle(snapshot);
            thread::sleep(Duration::from_millis(1000));
        }
    }
}

// TODO: delete me
fn dumb_function_tests() {
    let card_idm_str = "3500bd2ae3724e58";
    let card_idm = i64::from_str_radix(card_idm_str, 16).unwrap_or(0);

    let accesscode = card::get_008_accesscode(card_idm_str);
    let card_id_hex = format!("{:016X}", card_idm);

    println!("0008 access code: {accesscode}");
    println!("hex: {card_id_hex}");
}

enum PacketType {
    None,   // bad data
    CardScan,
    CoinInput,
    TestButton,
    ServiceButton,
    KeypadInput
}

impl From<u8> for PacketType {
    fn from(value: u8) -> Self {
        match value {
            1 => PacketType::CardScan,
            2 => PacketType::CoinInput,
            3 => PacketType::TestButton,
            4 => PacketType::ServiceButton,
            5 => PacketType::KeypadInput,
            _ => PacketType::None, // default case
        }
    }
}

fn main() {
    dumb_function_tests();

    println!("Waiting for game to launch...");

    let handle = find_game_instance();

    // TODO: make this configurable?
    // 1 13 21 = a m u
    let listener = TcpListener::bind("0.0.0.0:11321").unwrap();
    listener.set_nonblocking(true).unwrap();

    while handle.game_running() {
        // this *could* be multithreaded by simply copypasting
        // the implementation from ch21.3 of the rust book
        // but i'm lazy and realistically this code is not busy at all (job for later me) :)))
        match listener.accept() {
            Ok((stream, _)) => {
                let mut stream = stream;
                // i'll use protobuf or whatever if i need to expand features
                // this will work for now...

                // length = 9 (always)
                let mut buffer = [0u8; 17];

                // our data *always should be* 9 bytes, discard every other data.
                if stream.read(&mut buffer).unwrap_or(usize::MAX) != 17 {
                    eprintln!("Discarding Possibly Malformed Data (length != 17)");
                    continue;
                }

                // 1st byte (1 byte) - PacketType
                let packet_type = *buffer.get(0).unwrap_or(&0);
                // 2nd byte (8 bytes) - Data
                let data: [u8; 8] = buffer[1..9].try_into().unwrap();

                match PacketType::from(packet_type) {
                    PacketType::CardScan => {
                        // we need to convert to a string because that's what spice wants
                        // (is convenient for me also)
                        // yes, later i do convert back to int but like whatever man
                        let card_idm = format!("{:016X}", i64::from_be_bytes(data));

                        let client_ip = stream.peer_addr().unwrap();
                        println!("Card Data Recieved from {client_ip}");
                        // TODO: delete this log probably.
                        println!("Logging in with {card_idm}");

                        handle.login(&card_idm);
                    }
                    PacketType::CoinInput => {
                        handle.add_coin();
                    }
                    PacketType::TestButton => {
                        handle.test();
                    }
                    PacketType::ServiceButton => {
                        handle.service();
                    }
                    PacketType::KeypadInput  => {
                        !todo!("implement keypad input");
                    }
                    _ => {
                        eprintln!("Incorrect Command, Discarding.");
                        continue;
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No incoming connections, sleep briefly and check game status
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => {
                eprintln!("Accept error: {}", e);
            }
        }
    }

    println!("Exiting as game exited.");
}
