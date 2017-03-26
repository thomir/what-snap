extern crate argparse;
extern crate hyper;
extern crate url;
extern crate serde_json;
extern crate xdg;

use argparse::{ArgumentParser, Store, StoreTrue};
use hyper::client::Client;
use url::{Url, ParseError};
use std::io::Read;
use std::io::Write;
use serde_json::Value;
use std::fs::File;
use std::result::Result;


fn main() {
    let mut snap_id = String::new();
    let mut staging = false;
    let mut verbose = false;
    {
        let mut parser = ArgumentParser::new();
        parser.set_description("Translate snap package IDs into snap package names.");
        parser.refer(&mut snap_id)
            .add_argument("snap_id", Store, "snap id to translate")
            .required();
        parser.refer(&mut staging).add_option(&["--staging"], StoreTrue, "Use Staging services.");
        parser.refer(&mut verbose).add_option(&["--verbose"], StoreTrue, "Be Verbose");

        parser.parse_args_or_exit();
    }

    let cache = Cache::new("what-snap");
    match cache.get_value_for_key(&snap_id) {
        Some(name) => {
            print_snap_name(&snap_id, &name);
        }
        None => {
            let url = get_url_for_snap_id(staging, &snap_id).unwrap();

            let client = Client::new();
            let res = client.get(url).send().unwrap();

            if res.status == hyper::Ok {
                let snap_name = extract_snap_name_from_json(res);
                print_snap_name(&snap_id, &snap_name);

                match cache.store_value_for_key(&snap_id, &snap_name) {
                    Err(error) => {
                        if verbose {
                            println!("Error storing cache value: {}", error);
                        }
                    }
                    Ok(_) => {}
                }
            } else {
                println!("Response was not OK: {}", res.status);
            }
        }
    }
}


fn get_url_for_snap_id(staging: bool, snap_id: &String) -> Result<Url, ParseError> {
    let assertion_service_url = if staging {
        "https://assertions.staging.ubuntu.com"
    } else {
        "https://assertions.ubuntu.com"
    };

    Url::parse(assertion_service_url)
        .and_then(|url| url.join("v1/assertions/snap-declaration/16/"))
        .and_then(|url| url.join(&*snap_id))

}

fn extract_snap_name_from_json<T: Read>(reader: T) -> String {
    let data: Value = serde_json::from_reader(reader).unwrap();
    let obj = data.as_object().unwrap();
    let headers = obj.get("headers").unwrap().as_object().unwrap();
    let snap_name = headers.get("snap-name").unwrap();
    return String::from(snap_name.as_str().unwrap());
}


fn print_snap_name(snap_id: &String, snap_name: &String) {
    println!("{}: {}", snap_id, snap_name);
}


// TODO - maybe move this into a separate crate?
struct Cache {
    storage: Option<xdg::BaseDirectories>,
}

impl Cache {
    fn new(application_name: &str) -> Cache {
        let xdg = xdg::BaseDirectories::with_prefix(application_name);
        if xdg.is_ok() {
            Cache { storage: Some(xdg.unwrap()) }
        } else {
            Cache { storage: None }
        }
    }

    fn get_value_for_key(&self, key: &String) -> Option<String> {
        match self.storage {
            None => None,
            Some(ref storage) => {
                let path = storage.find_cache_file(key);
                match path {
                    Some(path) => {
                        let mut buf = String::new();
                        let content_length = File::open(path)
                            .and_then(|mut f| f.read_to_string(&mut buf));
                        if content_length.is_ok() {
                            Some(buf)
                        } else {
                            None
                        }
                    }
                    None => None,
                }

            }
        }

    }

    fn store_value_for_key(&self, key: &String, value: &String) -> Result<(), String> {
        match self.storage {
            None => Err(String::from("No Cache Available")),
            Some(ref storage) => {
                storage.place_cache_file(key)
                    .and_then(|path| File::create(path))
                    .and_then(|mut file| write!(file, "{}", value))
                    .map_err(|err| err.to_string())
            }
        }
    }
}
