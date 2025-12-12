//! Database runtime with synchronous event loop and parallel procedures.
//!
//! This crate provides the main database loop running at configurable tickrate
//! (15-120 Hz) and parallel iteration capabilities.

pub mod parallel;
pub mod runtime;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
