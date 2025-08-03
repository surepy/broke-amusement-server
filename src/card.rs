// this file largely exists to handle sega games, because
// for spice i just plug in the idm to card0.txt

use std::{env, fs, path::Path};

// i do not have access to aimedb (and probably never will), so...
// we will send a web request to sega.bsnk.me -> permanently cache the result to 
// ``LOCALAPPDATA\brokeamu.cache.json``
// (so I won't bother the host too much if at all...)
fn get_aimedb_accesscode(idm: &str) {
    
    // TODO: cache web requests impl
    // directory used for caching sega.bsnk.me web requests
    let cache_dir_string = env::var("LOCALAPPDATA")
        // this really shouldn't fail, but
        .unwrap_or(String::from("./"));

    let cache_file_path = Path::new(&cache_dir_string).with_file_name("brokeamu.cache.json");
    let mut cache_file= fs::File::create(cache_file_path).unwrap();

    !todo!("Implement get_aimedb_accesscode")

    //cache_file.write(b"buf");
}

// returns a string representation of a 0008 access code from a given idm
// this is how majority of the "data service providers" handles cards, but
// is apprently a "really bad tm" way to handle it? 
// however this _really_ isn't my problem to solve at all, so this will probably 
// just be default
pub fn get_008_accesscode_i64(idm: i64) -> String {
    let mut string_rep = String::from("000");
    string_rep.push_str(&idm.to_string());
    return string_rep;
}

pub fn get_008_accesscode(card_idm_str: &str) -> String {
    return get_008_accesscode_i64(i64::from_str_radix(card_idm_str, 16).unwrap_or(0));
}