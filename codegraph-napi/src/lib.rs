#[macro_use]
extern crate napi_derive;

#[napi]
pub fn get_version() -> String {
    codegraph_core::get_version().to_string()
}
