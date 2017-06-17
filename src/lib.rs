extern crate nix;
extern crate regex;

pub mod process;
pub mod session;
pub mod reader;

pub use session::spawn;

#[macro_use]
extern crate error_chain;

#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;

pub mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain!{
        errors {
            EOF {
                description("An error occurred during startup")
                display("Startup aborted: did not complete successfully")
            }
        }
    }
}
