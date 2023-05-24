extern crate pkg_config;

fn main() {
    // doesn't work on ubuntu with lib ver 0.11.2
    let hidapi = pkg_config::probe_library("hidapi-hidraw").unwrap();
}
