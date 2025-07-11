use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use winapi::shared::minwindef::MAX_PATH;
use winapi::um::minwinbase::STILL_ACTIVE;
use winapi::um::processthreadsapi::GetExitCodeProcess;
use winapi::um::psapi::GetModuleFileNameExW;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::{INPUT_u, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_RETURN};

use crate::card::get_008_accesscode;

// checks if handle process is running, returns true if yes
fn is_process_running(handle: &HANDLE) -> bool {
    let mut exit_code: u32 = 0;
    unsafe {
        if GetExitCodeProcess(*handle, &mut exit_code) != 0 {
            // i hope spice or amdaemon never returns 259
            if exit_code != STILL_ACTIVE {
                return false;
            }
        }
    };

    true
}

fn get_exe_directory(process_handle: HANDLE) -> Option<PathBuf> {
    // MAX_PATH: if you're using extended path length or LongPathsEnabled
    // you can go fuck yourself hehe (wil fix????)
    let mut buffer = [0u16; MAX_PATH];

    unsafe {
        let len = GetModuleFileNameExW(
            process_handle,
            std::ptr::null_mut(),
            buffer.as_mut_ptr(),
            260,
        );
        // nSize as magix number and not as MAX_PATH as its not u32
        if len > 0 {
            let path_osstr = OsString::from_wide(&buffer[..len as usize]);
            let mut path = PathBuf::from(path_osstr);
            path.pop();
            return Some(path);
        }
    }
    None
}

pub trait GameInstance {
    fn login(&self, idm: &str);
    fn game_running(&self) -> bool;
}

// spice
pub struct SpiceGameInstance {
    game_handle: HANDLE,
}

impl SpiceGameInstance {
    pub fn new(hnd: HANDLE) -> SpiceGameInstance {
        SpiceGameInstance { game_handle: hnd }
    }
}

impl GameInstance for SpiceGameInstance {
    fn login(&self, idm: &str) {
        // SpiceGameInstance-specific login implementation
        // plans:
        // 1. find card0.txt
        // 2. write idm to card0.txt
        // 3. dynamically press the "scan card" button
        // 4. profit!!!!!!
        // plan b(etter):
        // 1. virtual hid driver
        // 2. hid driver sends card scan event
        !todo!()
    }

    fn game_running(&self) -> bool {
        is_process_running(&self.game_handle)
    }
}

pub struct SegaToolsInstance {
    game_handle: HANDLE,
    card_file: PathBuf,
}

impl SegaToolsInstance {
    pub fn new(hnd: HANDLE) -> SegaToolsInstance {
        let mut directory = get_exe_directory(hnd).unwrap();
        println!("using {} as exeDir", directory.display());

        // if segatools.ini isn't next to amdaemon, something is wrong and i don't like it...
        // I'm pretty sure segatools.ini is ALWAYS there but if not hey you crash for free.
        // ur fault for not following the community common setup :)
        directory.push("segatools.ini");

        let config_contents = fs::read_to_string(&directory).unwrap();

        // find aimepath= to write our card data to.
        let mut card_file: Option<&Path> = None;
        for line in config_contents.lines() {

            if line.starts_with("aimePath=") {
                directory.set_file_name(line.split("=").last().unwrap());

                println!("using {} as aimePath", directory.display());
                let aime_path = directory.as_path();

                // line is probably "aimePath=" (path null)
                // and that's not my issue so just panic
                if !aime_path.exists() {
                    panic!("error parsing '{line}'!");
                }

                card_file = Some(aime_path);
                break;
            }
        }

        // if aimePath isn't found at all we also crash.
        // but like if you don't have aimePath it's either
        // a. your config is wrong and we don't want to make assumptions
        // b. have a real card reader so it doesn't matter
        if card_file.is_none() {
            panic!("aimePath not found, check segatools.ini!");
        }

        SegaToolsInstance {
            game_handle: hnd,
            card_file: card_file.unwrap().to_path_buf(),
        }
    }
}

impl GameInstance for SegaToolsInstance {
    fn login(&self, idm: &str) {
        // TODO: get real access code settin using get_aimedb_accesscode
        //  see comments in get_008_accesscode
        //  for now this will do
        let access_code = get_008_accesscode(idm);
        
        // delete this?
        println!("SegaToolsInstance: got Access Code {}", access_code);

        let write_op = fs::write(&self.card_file, access_code);

        if write_op.is_err() {
            eprintln!("failed to write card information.");
            return;
        }

        // signal the game to read card data
        // this whole block looks really ugly, I blame microsoft.
        unsafe {
            let mut input_down = INPUT {
                type_: INPUT_KEYBOARD,
                u: {
                    let mut u = std::mem::zeroed::<INPUT_u>();
                    *u.ki_mut() = KEYBDINPUT {
                        wVk: VK_RETURN as u16,
                        wScan: 0,
                        dwFlags: 0,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                    u
                },
            };

            // card scan start
            SendInput(1, &mut input_down, std::mem::size_of::<INPUT>() as i32);
            
            // hold enter for idk a good 1.5 seconds
            // TODO: find out how long does it actually take for a card scan.
            thread::sleep(Duration::from_millis(1500));

            let mut input_up = INPUT {
                type_: INPUT_KEYBOARD,
                u: {
                    let mut u = std::mem::zeroed::<INPUT_u>();
                    *u.ki_mut() = KEYBDINPUT {
                        wVk: VK_RETURN as u16,
                        wScan: 0, 
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                    u
                },
            };

            // card scan end
            SendInput(1, &mut input_up, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn game_running(&self) -> bool {
        is_process_running(&self.game_handle)
    }
}
