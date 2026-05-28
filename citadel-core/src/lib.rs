pub mod types;
pub mod util;
pub mod constants;
pub mod error;
pub mod fs;
pub mod storage;
pub mod graph;
pub mod extraction;
pub mod resolution;
pub mod context;

pub fn get_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_version() {
        let v = get_version();
        assert!(!v.is_empty(), "version should not be empty");
        assert!(v.contains('.'), "version should contain dots");
    }
}
