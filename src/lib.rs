extern crate nix;

pub mod process;
pub mod session;

pub use session::spawn;

#[macro_use]
extern crate error_chain;

mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain!{}
}