//! Main module of rexpect: start new process and interact with it

use process::PtyProcess;
use std::io::{BufReader, LineWriter};
use std::ffi::OsStr;
use std::fs::File;
use std::process::Command;
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::prelude::*;
use nix::sys::{wait, signal};
use nix::unistd;
use errors::*; // load error-chain

/// Interact with a process with read/write/signals, etc.
pub struct PtySession {
    process: PtyProcess,
    writer: LineWriter<File>,
    reader: BufReader<File>,
}

impl PtySession {

    /// sends string and a newline to process
    ///
    /// this is guaranteed to be flushed to the process
    /// returns number of written bytes
    pub fn send_line(&mut self, line: &str) -> Result<(usize)> {
        let mut len = self.send(line)?;
        len += self.writer.write(&['\n' as u8]).chain_err(|| "cannot write newline")?;
        Ok(len)
    }

    /// sends string to process. This may be buffered. You may use flush() after send()
    /// returns number of written bytes
    pub fn send(&mut self, s: &str) -> Result<(usize)> {
        self.writer.write(s.as_bytes()).chain_err(|| "cannot write line to process")
    }

    /// make sure all bytes written via `send()` are sent to the process
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush().chain_err(|| "could not flush")
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
    ///     let process = rexpect::spawn("/usr/bin/myprog").expect("cannot run cat");
    ///     while process.status().unwrap() == wait::WaitStatus::StillAlive {
    ///         // do something
    ///     }
    /// # }
    /// ```
    ///
    pub fn status(&self) -> Result<(wait::WaitStatus)> {
        wait::waitpid(self.process.child_pid, Some(wait::WNOHANG)).chain_err(|| "cannot read status")
    }

    /// Wait until process has exited. This is a blocking call.
    /// If the process doesn't terminate this will block forever.
    pub fn wait(&self) ->Result<(wait::WaitStatus)> {
        wait::waitpid(self.process.child_pid, None).chain_err(|| "wait: cannot read status")
    }

    /// regularly exit the process
    ///
    /// closes the pty session and sends SIGTERM to the process
    pub fn exit(&self) -> Result<()> {
        self.kill(signal::SIGTERM)
    }

    /// kills the process with a specific signal
    ///
    /// closes the pty session and sends SIGTERM to the process
    pub fn kill(&self, sig:signal::Signal) -> Result<()> {
        unistd::close(self.process.pty.as_raw_fd()).and_then(|_|
            signal::kill(self.process.child_pid, sig)
        ).chain_err(|| "failed to exit process")
    }
}

pub fn spawn<S: AsRef<OsStr>>(program: S) -> Result<PtySession> {
    let command = Command::new(program);
    let process = PtyProcess::new(command).chain_err(|| "couldn't start process")?;
    let f = unsafe { File::from_raw_fd(process.pty.as_raw_fd()) };
    let writer = LineWriter::new(f.try_clone().chain_err(|| "couldn't open write stream")?);
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
    #[test]
    fn test_cat2() {
        || -> Result<()> {
            let mut s = spawn("cat")?;
            s.send_line("hans")?;
            s.exit()?;
            println!("status={:?}", s.wait()?);
            Ok(())
        }().expect("could not execute");
    }

}
