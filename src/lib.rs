//! The main crate of Rexpect
//!
//! # Overview
//!
//! Rexpect is a loose port of [pexpect](pexpect.readthedocs.io/en/stable/)
//! which itself is inspired by Don Libe's expect.
//!
//! It's main components (depending on your need you can use either of those)
//!
//! - [session](session/index.html): automate stuff in Rust
//! - [reader](reader/index.html): a non-blocking reader with buffering, matching on
//!   strings/regex/...
//! - [process](process/index.html): spawn a process in a pty
//!
//! # Basic example
//!
//! ```no_run
//!
//! extern crate rexpect;
//!
//! use rexpect::spawn;
//! use rexpect::Result;
//!
//! fn do_ftp() -> Result<()> {
//!     let mut p = spawn("ftp speedtest.tele2.net", Some(2000))?;
//!     p.exp_regex("Name \\(.*\\):")?;
//!     p.send_line("anonymous")?;
//!     p.exp_string("Password")?;
//!     p.send_line("test")?;
//!     p.exp_string("ftp>")?;
//!     p.send_line("cd upload")?;
//!     p.exp_string("successfully changed.\r\nftp>")?;
//!     p.send_line("pwd")?;
//!     p.exp_regex("[0-9]+ \"/upload\"")?;
//!     p.send_line("exit")?;
//!     p.exp_eof()?;
//!     Ok(())
//! }
//!
//!
//! fn main() {
//!     do_ftp().unwrap_or_else(|e| panic!("ftp job failed with {}", e));
//! }
//! ```
//!
//! # Example with bash
//!
//! Tip: try the chain of commands first in a bash session.
//! The tricky thing is to get the wait_for_prompt right.
//! What `wait_for_prompt` actually does is seeking to the next
//! visible prompt. If you forgot to call this once your next call to
//! `wait_for_prompt` comes out of sync and you're seeking to a prompt
//! printed "above" the last `execute()`.
//!
//! ```no_run
//! extern crate rexpect;
//! use rexpect::spawn_bash;
//! use rexpect::Result;
//!
//!
//! fn run() -> Result<()> {
//!     let mut p = spawn_bash(Some(30_000))?;
//!     p.execute("ping 8.8.8.8", "bytes of data")?;
//!     p.send_control('z')?;
//!     p.wait_for_prompt()?;
//!     p.execute("bg", "suspended")?;
//!     p.send_line("sleep 1")?;
//!     p.wait_for_prompt()?;
//!     p.execute("fg", "continued")?;
//!     p.send_control('c')?;
//!     p.exp_string("packet loss")?;
//!     Ok(())
//! }
//!
//! fn main() {
//!     run().unwrap_or_else(|e| panic!("bash process failed with {}", e));
//! }
//!
//! ```

pub mod process;
pub mod session;
pub mod reader;

pub use session::{spawn, spawn_bash, spawn_python, spawn_stream};
pub use reader::ReadUntil;

use std::time;
use thiserror::Error;

///Simplify result type
pub type Result<T> = std::result::Result<T, Error>;

#[non_exhaustive]
#[derive(Error, Debug)]
pub enum Error {
    ///The pipe to the process is broken. Most probably because
    ///the process died.
    #[non_exhaustive]
    #[error("PipeError")]
    BrokenPipe,

    ///The provided program name is empty.
    #[non_exhaustive]
    #[error("EmptyProgramName")]
    EmptyProgramName,

    ///Error communicating with PytProcess
    #[non_exhaustive]
    #[error("There was an IO error. {}", context)]
    IOError {
        context: String,
        source: std::io::Error,
    },

    ///There was some other PtyProcess error.
    #[non_exhaustive]
    #[error("There was PtyError. {}", context)]
    PtyError { context: String, source: nix::Error },

    ///Invalid regular expression
    #[non_exhaustive]
    #[error("Invalid regular expression. {}", regex)]
    RegexError { regex: String, source: regex::Error },

    ///End of files stream (usually stdout) occurred, most probably
    ///because the process terminated.
    #[non_exhaustive]
    #[error("EOF (End of File): Expected {} but got EOF after reading \"{}\", \
    process terminated with {:?}", expected, got,
    exit_code.as_ref()
    .unwrap_or(& "unknown".to_string()))]
    EOF {
        expected: String,
        got: String,
        exit_code: Option<String>,
    },

    ///The process didn't end within the given timeout
    #[non_exhaustive]
    #[error("Timeout Error: Expected {} but got \"{}\" (after waiting {} ms)",
    expected, got, (timeout.as_secs() * 1000) as u32
    + timeout.subsec_nanos() / 1_000_000)]
    Timeout {
        expected: String,
        got: String,
        timeout: time::Duration,
    },

    ///User is attempting to use an unknown control character.
    #[non_exhaustive]
    #[error("Unknown Control Character Ctrl-{}",.0)]
    UnknownControlChar(char),
}
