//! Disk persistence and schema serialization.
//!
//! This crate handles binary snapshots to disk and JSON schema serialization.

pub mod snapshot;
pub mod serialization;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}