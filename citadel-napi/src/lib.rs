#[macro_use]
extern crate napi_derive;

mod database;
mod extraction;

pub use database::Database;
pub use extraction::extract_files;

#[napi]
pub fn get_version() -> String {
    citadel_core::get_version().to_string()
}
