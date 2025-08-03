use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::{env, thread};
use std::time::Duration;

use winapi::shared::minwindef::MAX_PATH;
use winapi::um::minwinbase::STILL_ACTIVE;
use winapi::um::processthreadsapi::GetExitCodeProcess;
use winapi::um::psapi::GetModuleFileNameExW;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::{
    INPUT, INPUT_KEYBOARD, INPUT_u, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_RETURN,
};

use crate::card::{get_008_accesscode, get_aimedb_accesscode};

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

// SendInput wrapper
fn keybd_input(key: i32) {
    // this whole block looks really ugly, I blame microsoft.
    unsafe {
        let mut input_down = INPUT {
            type_: INPUT_KEYBOARD,
            u: {
                let mut u = std::mem::zeroed::<INPUT_u>();
                *u.ki_mut() = KEYBDINPUT {
                    wVk: key as u16,
                    wScan: 0,
                    dwFlags: 0,
                    time: 0,
                    dwExtraInfo: 0,
                };
                u
            },
        };

        SendInput(1, &mut input_down, std::mem::size_of::<INPUT>() as i32);

        // gutentight value of sleep before lifting the key
        thread::sleep(Duration::from_millis(1500));

        let mut input_up = INPUT {
            type_: INPUT_KEYBOARD,
            u: {
                let mut u = std::mem::zeroed::<INPUT_u>();
                *u.ki_mut() = KEYBDINPUT {
                    wVk: key as u16,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                };
                u
            },
        };

        SendInput(1, &mut input_up, std::mem::size_of::<INPUT>() as i32);
    };
}

pub trait GameInstance {
    fn login(&self, idm: &str);
    fn add_coin(&self);
    fn test(&self);
    fn service(&self);
    fn game_running(&self) -> bool;
}

// spice
pub struct SpiceGameInstance {
    game_handle: HANDLE,
    card_file: PathBuf,
    // unimplemented
    coin_key: i32,
}

impl SpiceGameInstance {
    pub fn new(hnd: HANDLE) -> SpiceGameInstance {
        SpiceGameInstance {
            game_handle: hnd,
            card_file: todo!(),
            coin_key: todo!(),
        }
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

    fn add_coin(&self) {
        todo!()
    }

    fn test(&self) {
        todo!()
    }

    fn service(&self) {
        todo!()
    }
}

pub struct SegaToolsInstance {
    game_handle: HANDLE,
    card_file: PathBuf,
    coin_key: i32,
    test_key: i32,
    service_key: i32,
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

        // defaults
        let test_key = 0x70;
        let service_key = 0x71;
        let coin_key = 0x72;

        // find aimepath= to write our card data to.
        let mut card_file: Option<&Path> = None;
        for line in config_contents.lines() {
            if line.starts_with("aimePath=") {
                directory.set_file_name(line.split("=").last().unwrap());

                println!("using {} as aimePath", directory.display());
                let aime_path = directory.as_path();

                // line is probably "aimePath=" (path null)
                // aka unconfigured.
                // and that's not my issue so just panic and kill the program
                if !aime_path.exists() {
                    panic!("error parsing '{line}'!");
                }

                card_file = Some(aime_path);
                break;
            }
            // io3.test
            // TODO: make configurable
            else if line.starts_with("test=") {
                let d = line.split("=").last().unwrap();
            }
        }

        // if aimePath isn't found at all we crash also.
        // but like if you don't have aimePath it's either
        // a. your config is wrong and we don't want to make assumptions
        // b. have a real card reader so it doesn't matter
        if card_file.is_none() {
            panic!("aimePath not found, check segatools.ini!");
        }

        SegaToolsInstance {
            game_handle: hnd,
            card_file: card_file.unwrap().to_path_buf(),
            coin_key,
            test_key,
            service_key,
        }
    }
}

impl GameInstance for SegaToolsInstance {
    fn login(&self, idm: &str) {
        let mut access_code= get_008_accesscode(idm);
        
        // try to get the more canonically correct acccesscode, if requested.
        // OPINION: i don't think you should use this
        // because all the other card readers and data providers do it wrong anyway
        let try_aimedb = env::var("BAS_SEGA_TRY_AIMEDB").is_ok();
        if try_aimedb {
            access_code = get_aimedb_accesscode(idm);
        }

        // delete this?
        println!("SegaToolsInstance: got Access Code {}", access_code);

        let write_op = fs::write(&self.card_file, access_code);

        if write_op.is_err() {
            eprintln!("failed to write card information.");
            return;
        }

        // the enter key is default and unconfigurable in segatools
        // i think
        keybd_input(VK_RETURN);
    }

    fn game_running(&self) -> bool {
        is_process_running(&self.game_handle)
    }

    fn add_coin(&self) {
        keybd_input(self.coin_key)
    }

    fn test(&self) {
        keybd_input(self.test_key)
    }

    fn service(&self) {
        keybd_input(self.service_key)
    }
}
