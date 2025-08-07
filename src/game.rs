use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, thread};

use winapi::shared::minwindef::MAX_PATH;
use winapi::um::minwinbase::STILL_ACTIVE;
use winapi::um::processthreadsapi::GetExitCodeProcess;
use winapi::um::psapi::GetModuleFileNameExW;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::{
    INPUT_u, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_F1, VK_F2, VK_F3, VK_RETURN, VK_SPACE
};

use quick_xml::Reader as XMLReader;
use quick_xml::events::Event as XMLEvent;

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
    // each key press is a thread, what could possibly go wrong
    thread::spawn(move || {
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
            thread::sleep(Duration::from_millis(500));

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
    });
}

pub trait GameInstance {
    fn login(&self, idm: &str);
    fn add_coin(&self);
    fn test(&self);
    fn service(&self);
    fn game_running(&self) -> bool;
}

// spice
enum SpiceGameType {
    None, // lol
    SoundVortex,
}

pub struct SpiceGameInstance {
    game_handle: HANDLE,
    card_file: PathBuf,
    // unimplemented
    game_type: SpiceGameType,
}

impl From<&str> for SpiceGameType {
    fn from(value: &str) -> Self {
        match value {
            "KFC" => SpiceGameType::SoundVortex,
            _ => SpiceGameType::None, // default case
        }
    }
}

fn spice_config_game_name(value: &SpiceGameType) -> &str {
    match value {
        SpiceGameType::SoundVortex => &"Sound Voltex",
        _ => "",
    }
}
/// finds a key in str
/// assumes the end is a str
fn xml_config_entry_str(content: &str, key: &str) -> String {
    let mut config_reader = XMLReader::from_str(&content);
    config_reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_path = Vec::new();

    loop {
        match config_reader.read_event_into(&mut buf) {
            Err(e) => panic!(
                "Error at position {}: {:?}",
                config_reader.error_position(),
                e
            ),
            Ok(XMLEvent::Eof) => break,
            Ok(XMLEvent::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                current_path.push(name);
            },
            Ok(XMLEvent::End(_)) => { current_path.pop(); },
            Ok(XMLEvent::Text(e)) => {
                let path = current_path.join(".");
                if path == key {
                    return e.decode().unwrap().into_owned();
                }
            }
            _ => {}
        }
        buf.clear();
    }

    // return "" if not found
    String::new()
}

impl SpiceGameInstance {
    pub fn new(hnd: HANDLE) -> SpiceGameInstance {
        // TODO replace with get_exe_directory(hnd).unwrap();
        let mut directory = get_exe_directory(hnd).unwrap();

        println!("using {} as exeDir", directory.display());

        // spice games... some games have more than 1 card reader (see: IIDX)
        //  but this is only "one reader" and i dont really think this app is for 2p play
        //  ... and the people that want 2p play probably have an actual cab so WONTFIX
        let mut card_file = directory.clone();
        card_file.push("card0.txt");

        // figure out what game we're running
        // TODO props/ea3-config.xml
        directory.push("ea3-config.xml");

        let ea3_config_str = fs::read_to_string(&directory).unwrap();
        let game_type  = SpiceGameType::from(
            xml_config_entry_str(&ea3_config_str, "ea3.soft.model").as_str()
        );

        println!("Detected Game {}",  spice_config_game_name(&game_type));

        SpiceGameInstance {
            game_handle: hnd,
            card_file,
            game_type,
        }
    }
}

impl GameInstance for SpiceGameInstance {
    // FIXME: because in spice you can actually *remap the buttons*
    // we are stuck with a situation where where we probably have to 
    // read the spicetools config every time we press a button
    // this really sucks and i don't wanna deal with it
    // so we have "best guess defaults"
    // INFO: if key=255 it means unbinded

    // later: 
    // let spice_config_file = Path::new(&env::var("APPDATA").unwrap()).join("spicetools.xml");

    fn login(&self, idm: &str) {
        // plans:
        // 1. find card0.txt
        // 2. write idm to card0.txt
        // 3. dynamically press the "scan card" button
        // 4. profit!!!!!!
        // TODO plan b(etter):
        // 1. virtual hid driver
        // 2. hid driver sends card scan event

        // for spice we can just dump the idm and it works
        let write_op = fs::write(&self.card_file, idm);

        if write_op.is_err() {
            eprintln!("failed to write card information.");
            return;
        }

        keybd_input(VK_SPACE);
    }

    fn game_running(&self) -> bool {
        is_process_running(&self.game_handle)
    }

    fn test(&self) {
        keybd_input(VK_F1)
    }

    fn service(&self) {
        keybd_input(VK_F2)
    }
    
    fn add_coin(&self) {
        keybd_input(VK_F3)
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
        let mut access_code = get_008_accesscode(idm);

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
