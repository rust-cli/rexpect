//! Main module of rexpect: start new process and interact with it

use process::PtyProcess;
use std::io::{BufReader, LineWriter, Result};
use std::ffi::OsStr;
use std::fs::File;
use std::process::Command;
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::prelude::*;
use nix::sys::{wait, signal};
use nix::unistd;
use nix;

/// Interact with a process with read/write/signals, etc.
pub struct PtySession {
    process: PtyProcess,
    writer: LineWriter<File>,
    reader: BufReader<File>,
}

impl PtySession {
    pub fn send_line(&mut self, line: &str) -> Result<()> {
        self.writer.write_all(line.as_bytes())
    }

    /// get status of child process, nonblocking
    ///
    /// # Example
    /// ```rust,no_run
    ///
    /// # extern crate nix;
    /// # extern crate rexpect;
    /// # use nix::sys::wait;
    ///
    /// # fn main() {
    ///     let process = rexpect::spawn("sleep 5").expect("cannot run cat");
    ///     while process.status() == Ok(wait::WaitStatus::StillAlive) {
    ///         // do something
    ///     }
    /// # }
    /// ```
    ///
    pub fn status(&self) -> nix::Result<(wait::WaitStatus)> {
        wait::waitpid(self.process.child_pid, Some(wait::WNOHANG))
    }

    /// regularly exit the process
    ///
    /// sends SIGHUP and closes the pty session
    pub fn exit(&self) -> nix::Result<()> {
        signal::kill(self.process.child_pid, signal::SIGHUP).and_then(|_|
            unistd::close(self.process.pty.as_raw_fd())
        )
    }
}

pub fn spawn<S: AsRef<OsStr>>(program: S) -> Result<PtySession> {
    let command = Command::new(program);
    let process = PtyProcess::new(command)?;
    let f = unsafe { File::from_raw_fd(process.pty.as_raw_fd()) };
    let writer = LineWriter::new(f.try_clone()?);
    let reader = BufReader::new(f);
    Ok(PtySession {
           process: process,
           writer: writer,
           reader: reader,
       })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep_ms;
    #[test]
    fn test_cat() {
        || -> Result<()> {
            let mut s = spawn("cat")?;
            s.send_line("hans")?;
            s.exit()?;
            println!("status={:?}", s.status()?);
            Ok(())
        }().expect("could not execute");
    }

}
