#[macro_use]
extern crate napi_derive;

use codegraph_core;

#[napi]
pub fn get_version() -> String {
    codegraph_core::get_version().to_string()
}
