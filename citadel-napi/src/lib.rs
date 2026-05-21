#[macro_use]
extern crate napi_derive;

mod database;

pub use database::Database;

#[napi]
pub fn get_version() -> String {
    citadel_core::get_version().to_string()
}
