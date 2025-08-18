// this file largely exists to handle sega games, because
// for spice i just plug in the idm to card0.txt

use std::{collections::HashMap, env, fs, io::{Read, Write}, path::Path};
use serde_json;

/// 
/// gets a more correct aimedb access code from card.bsnk.me
/// permanently caches the results to ``LOCALAPPDATA\brokeamu.aimedb_cache.json``
/// 
pub fn get_aimedb_accesscode(idm: &str) -> String {
    let cache_dir_string = env::var("LOCALAPPDATA")
        .unwrap_or(String::from("./"));

    // save the access code so we bother the host less (this result doesn't change anyway)
    let cache_file_path = Path::new(&cache_dir_string).join("brokeamu.aimedb_cache.json");
    
    // TODO probably don't load the entire cache file *every* function call
    // but this will do for now; i mean how big this file can possibly be anyway 
    let mut cache: HashMap<String, String> = if cache_file_path.exists() {
        let mut file = fs::File::open(&cache_file_path).unwrap_or_else(|_| {
            fs::File::create(&cache_file_path).unwrap()
        });
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap_or(0);
        serde_json::from_str(&contents).unwrap_or_else(|_| HashMap::new())
    } else {
        HashMap::new()
    };

    // Check if we already have this IDM cached
    if let Some(cached_accesscode) = cache.get(idm) {
        println!("get_aimedb_accesscode: cache hit");
        return cached_accesscode.clone();
    }

    println!("get_aimedb_accesscode: cache miss, calling card api.");
    // https://sega.bsnk.me/misc/card_convert/
    let url = format!("https://card.bsnk.me/normalise/sega:{}", idm);
    let response = reqwest::blocking::get(&url);
    
    let accesscode = match response {
        Ok(resp) => {
            if resp.status().is_success() {
                // there must be a reason, but the response is always wrapped in quotes
                // this is slightly annoying and makes this line ugly,
                // but it is what it is.
                let result_string = resp.text();
                if result_string.is_ok() {
                    result_string.unwrap()[1..21].to_string()
                }
                else {
                    get_008_accesscode(idm)
                }
            } else {
                get_008_accesscode(idm)
            }
        }
        Err(err) => {
            // we failed to even send the web request, so don't even
            // bother caching this result and return early
            eprintln!("get_aimedb_accesscode: request failure! ({err})");
            return get_008_accesscode(idm);
        }
    };

    println!("get_aimedb_accesscode: got accesscode {accesscode}");

    // Cache the result
    cache.insert(idm.to_string(), accesscode.clone());
    
    if let Ok(cache_json) = serde_json::to_string_pretty(&cache) {
        if let Ok(mut file) = fs::File::create(&cache_file_path) {
            let _ = file.write_all(cache_json.as_bytes());
        }
    }

    accesscode
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