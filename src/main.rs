use std::ffi::CStr;
use std::thread;
use std::time::Duration;
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::tlhelp32::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use winapi::um::winnt::PROCESS_QUERY_INFORMATION;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

mod game;
use crate::game::{GameInstance, SegaToolsInstance, SpiceGameInstance};
mod card;

fn create_game_instance(process_name: &str, entry: &PROCESSENTRY32W) -> Option<Box<dyn GameInstance>> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, 0, entry.th32ProcessID) };
    if handle.is_null() {
        return None;
    }
    
    match process_name {
        "spice.exe" | "spice64.exe" => Some(Box::new(SpiceGameInstance::new(handle))),
         // I dont know if amdaemon doesnt run on other games
         // I know that it runs on chunithm and ongeki /shrug
        "amdaemon.exe" => Some(Box::new(SegaToolsInstance::new(handle))),
        _ => None,
    }
}

// loops until it can find a game handle under
// "spice64.exe" or "amdaemon.exe"
fn find_game_instance() -> Box<dyn GameInstance> {
    loop {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);

            // why it do that
            if snapshot == winapi::um::handleapi::INVALID_HANDLE_VALUE {
                thread::sleep(Duration::from_millis(1000));
                continue;
            }

            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let process_name_wide = &entry.szExeFile[..entry.szExeFile.iter()
                        .position(|&c| c == 0).unwrap_or(entry.szExeFile.len())];
                    let process_name = OsString::from_wide(process_name_wide)
                        .to_string_lossy()
                        .to_lowercase();

                    if let Some(instance) = create_game_instance(&process_name, &entry) {
                        CloseHandle(snapshot);
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
fn dumb_function_tests () {
    let card_idm_str = "0100000000000";
    let card_idm = i64::from_str_radix(card_idm_str, 16).unwrap_or(0);

    let accesscode = card::get_008_accesscode(card_idm_str);
    let card_id_hex = format!("{:X}", card_idm);

    println!("0008 access code: {accesscode}");
    println!("hex: {card_id_hex}");
}

fn main() {
    dumb_function_tests();

    println!("Waiting for game to launch...");

    let handle = find_game_instance();

    while handle.game_running() {
        //
    }

    println!("Exiting as game exited.");
}

