use std::ffi::OsString;
use std::fs;
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
    INPUT, INPUT_KEYBOARD, INPUT_u, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_RETURN,
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
    // if key=255 it means unbinded in spice, so..
    // this creates a case where if some dude uses sega and has
    // one of the buttons as 255 it wont work...
    // too bad!
    if key == 255 {
        return;
    }

    // each key press is a thread, what could possibly go wrong
    // FIXME: put this into a thread queue, low priority.
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
    fn misc_keys(&self, key: i32);
}

// spice
#[derive(PartialEq)]
enum SpiceGameType {
    Unsupported, // lol
    SoundVortex,
    IIDX,
    Jubeat,
}

pub struct SpiceGameInstance {
    game_handle: HANDLE,
    card_file: PathBuf,
    // unimplemented
    game_type: SpiceGameType,

    coin_key: i32,
    test_key: i32,
    service_key: i32,

    // FIXME: because in spice you can actually *remap the buttons in-game*
    //  we are stuck with a situation where the buttons in-game might be
    //  different from what we have set here
    //  this sucks, but people don't change the keys in-game a lot so
    //  later you can watch spicetools.xml and reload as needed
    // WONTFIX: we can't detect a difference between Naive and "Bind"ed key
    //  ... well we can i just won't deal with it
    p1_keypad_0: i32,
    p1_keypad_1: i32,
    p1_keypad_2: i32,
    p1_keypad_3: i32,
    p1_keypad_4: i32,
    p1_keypad_5: i32,
    p1_keypad_6: i32,
    p1_keypad_7: i32,
    p1_keypad_8: i32,
    p1_keypad_9: i32,
    p1_keypad_00: i32,
    // what does this do
    p1_keypad_decimal: i32,
    p1_keypad_insert_card: i32,
}

impl From<&str> for SpiceGameType {
    fn from(value: &str) -> Self {
        match value {
            "KFC" => SpiceGameType::SoundVortex,
            "LDJ" => SpiceGameType::IIDX,
            "L44" => SpiceGameType::Jubeat,
            _ => SpiceGameType::Unsupported, // default case
        }
    }
}

fn spice_config_game_name(value: &SpiceGameType) -> &str {
    match value {
        SpiceGameType::SoundVortex => &"Sound Voltex",
        SpiceGameType::IIDX => &"Beatmania IIDX",
        SpiceGameType::Jubeat => &"Jubeat",
        _ => "Unsupported",
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
            }
            Ok(XMLEvent::End(_)) => {
                current_path.pop();
            }
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

        println!("Using {} as exeDir", directory.display());

        // WONTFIX: spice games... some games have more than 1 card reader (see: IIDX)
        //  but this is only "one reader" and i dont really think this app is for 2p play
        //  ... and the people that want 2p play probably have an actual cab so
        let mut card_file = directory.clone();
        card_file.push("card0.txt");

        println!("Using {} as card0.txt", card_file.display());

        // figure out what game we're running
        directory.push("prop/ea3-config.xml");

        let ea3_config_str = fs::read_to_string(&directory).unwrap();
        let game_type =
            SpiceGameType::from(xml_config_entry_str(&ea3_config_str, "ea3.soft.model").as_str());
        let game_type_str = spice_config_game_name(&game_type);

        println!("Detected Game={}", game_type_str);

        if game_type == SpiceGameType::Unsupported {
            eprintln!("! Running on unsupported game; config will not be loaded correctly!");
        }

        // These defaults are pulled from spice.
        let mut p1_keypad_0 = 0x60;
        let mut p1_keypad_1: i32 = 0x61;
        let mut p1_keypad_2: i32 = 0x62;
        let mut p1_keypad_3: i32 = 0x63;
        let mut p1_keypad_4: i32 = 0x64;
        let mut p1_keypad_5: i32 = 0x65;
        let mut p1_keypad_6: i32 = 0x66;
        let mut p1_keypad_7: i32 = 0x67;
        let mut p1_keypad_8: i32 = 0x68;
        let mut p1_keypad_9: i32 = 0x69;
        let mut p1_keypad_00: i32 = 0xD;
        let mut p1_keypad_decimal: i32 = 0x6E;
        let mut p1_keypad_insert_card: i32 = 0x6E;

        // these don't really have a defualt, but are good defaults (in my opinion)
        let mut test_key = 0x31;
        let mut service_key = 0x32;
        let mut coin_key = 0x33;

        let spice_config_file = Path::new(&env::var("APPDATA").unwrap()).join("spicetools.xml");
        let spice_config_str = fs::read_to_string(&spice_config_file).unwrap();

        let mut spice_config_reader = XMLReader::from_str(&spice_config_str);
        spice_config_reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        // we are looking at the correct <game> block
        let mut is_current_game = false;

        loop {
            match spice_config_reader.read_event_into(&mut buf) {
                Err(e) => panic!(
                    "Error at position {}: {:?}",
                    spice_config_reader.error_position(),
                    e
                ),
                Ok(XMLEvent::Eof) => break,
                Ok(XMLEvent::Start(e)) => {
                    if e.name().as_ref() != b"game" {
                        continue;
                    }

                    match e.try_get_attribute("name") {
                        Ok(Some(e)) => {
                            if String::from_utf8_lossy(&e.value).as_ref() == game_type_str {
                                is_current_game = true;
                            }
                        }
                        _ => continue,
                    }
                }
                Ok(XMLEvent::End(e)) => {
                    if e.name().as_ref() == b"game" && is_current_game {
                        break;
                    }
                }
                Ok(XMLEvent::Empty(e)) => {
                    if !is_current_game || e.name().as_ref() != b"button" {
                        continue;
                    }

                    let key = match e.try_get_attribute("vkey") {
                        Ok(Some(e)) => {
                            let key_str = String::from_utf8_lossy(&e.value).into_owned();

                            key_str.parse().unwrap_or(255)
                        }
                        _ => 255,
                    };

                    match e.try_get_attribute("name") {
                        Ok(Some(e)) => {
                            // long boyo
                            match String::from_utf8_lossy(&e.value).as_ref() {
                                "Service" => {
                                    service_key = key;
                                }
                                "Test" => {
                                    test_key = key;
                                }
                                "Coin Mech" => {
                                    coin_key = key;
                                }
                                "P1 Keypad 0" => {
                                    p1_keypad_0 = key;
                                }
                                "P1 Keypad 1" => {
                                    p1_keypad_1 = key;
                                }
                                "P1 Keypad 2" => {
                                    p1_keypad_2 = key;
                                }
                                "P1 Keypad 3" => {
                                    p1_keypad_3 = key;
                                }
                                "P1 Keypad 4" => {
                                    p1_keypad_4 = key;
                                }
                                "P1 Keypad 5" => {
                                    p1_keypad_5 = key;
                                }
                                "P1 Keypad 6" => {
                                    p1_keypad_6 = key;
                                }
                                "P1 Keypad 7" => {
                                    p1_keypad_7 = key;
                                }
                                "P1 Keypad 8" => {
                                    p1_keypad_8 = key;
                                }
                                "P1 Keypad 9" => {
                                    p1_keypad_9 = key;
                                }
                                "P1 Keypad 00" => {
                                    p1_keypad_00 = key;
                                }
                                "P1 Keypad Decimal" => {
                                    p1_keypad_decimal = key;
                                }
                                "P1 Keypad Insert Card" => {
                                    p1_keypad_insert_card = key;
                                }
                                _ => continue,
                            }
                        }
                        _ => continue,
                    }
                }
                _ => {}
            }
            buf.clear();
        }

        SpiceGameInstance {
            game_handle: hnd,
            card_file,
            game_type,
            coin_key,
            test_key,
            service_key,
            p1_keypad_0,
            p1_keypad_1,
            p1_keypad_2,
            p1_keypad_3,
            p1_keypad_4,
            p1_keypad_5,
            p1_keypad_6,
            p1_keypad_7,
            p1_keypad_8,
            p1_keypad_9,
            p1_keypad_00,
            p1_keypad_decimal,
            p1_keypad_insert_card,
        }
    }
}

impl GameInstance for SpiceGameInstance {
    fn login(&self, idm: &str) {
        // plans:
        // 1. find card0.txt
        // 2. write idm to card0.txt
        // 3. dynamically press the "scan card" button
        // 4. profit!!!!!!
        // TODO plan b(etter):
        // 1. virtual hid driver (find out how cardio works)
        // 2. hid driver sends card scan event

        // for spice we can just dump the idm and it works
        let write_op = fs::write(&self.card_file, idm);

        if write_op.is_err() {
            eprintln!("failed to write card information.");
            return;
        }

        keybd_input(self.p1_keypad_insert_card);
    }

    fn game_running(&self) -> bool {
        is_process_running(&self.game_handle)
    }

    fn test(&self) {
        keybd_input(self.test_key)
    }

    fn service(&self) {
        keybd_input(self.service_key)
    }

    fn add_coin(&self) {
        keybd_input(self.coin_key)
    }

    fn misc_keys(&self, key: i32) {
        keybd_input(
            // TODO: document/struct what "1", "2" is
            match key {
                1 => self.p1_keypad_1,
                2 => self.p1_keypad_2,
                3 => self.p1_keypad_3,
                4 => self.p1_keypad_4,
                5 => self.p1_keypad_5,
                6 => self.p1_keypad_6,
                7 => self.p1_keypad_7,
                8 => self.p1_keypad_8,
                9 => self.p1_keypad_9,
                10 => self.p1_keypad_0,
                11 => self.p1_keypad_00,
                12 => self.p1_keypad_decimal,
                _ => 255,
            },
        );
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
                let _d = line.split("=").last().unwrap();
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

    fn misc_keys(&self, _key: i32) {
        // Sega doesn't have a keypad so
    }
}
