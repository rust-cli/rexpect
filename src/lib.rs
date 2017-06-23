extern crate nix;
extern crate regex;

pub mod process;
pub mod session;
pub mod reader;

pub use session::spawn;

#[macro_use]
extern crate error_chain;

pub mod errors {
    // Create the Error, ErrorKind, ResultExt, and Result types
    error_chain!{
        errors {
            EOF {
                description("End of filestream (usually stdout) occurred, most probably because the process terminated")
                display("EOF (End of File)")
            }
            BrokenPipe {
                description("The pipe to the process is broken. Most probably because the process died.")
                display("PipeError")
            }
            Timeout {
                description("The process didn't end within the given timeout")
                display("Timeout Error")
            }
        }
    }
}
