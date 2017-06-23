//! Main module of rexpect: start new process and interact with it

use process::PtyProcess;
use reader::{NBReader, ReadUntil};
use std::io::LineWriter;
use std::fs::File;
use std::process::Command;
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::prelude::*;
use errors::*; // load error-chain

/// Interact with a process with read/write/signals, etc.
#[allow(dead_code)]
pub struct PtySession {
    process: PtyProcess,
    writer: LineWriter<File>,
    reader: NBReader,
    commandname: String, // only for debugging purposes now
}

/// Start a process in a tty session, write and read from it
///
/// # Example
///
/// ```
///
/// use rexpect::spawn;
/// # use rexpect::errors::*;
///
/// # fn main() {
///     # || -> Result<()> {
/// let mut s = spawn("cat", None)?;
/// s.send_line("hello, polly!")?;
/// let line = s.read_line()?;
/// assert_eq!("hello, polly!\r\n", line);
///         # Ok(())
///     # }().expect("test failed");
/// # }
/// ```

impl PtySession {
    /// sends string and a newline to process
    ///
    /// this is guaranteed to be flushed to the process
    /// returns number of written bytes
    pub fn send_line(&mut self, line: &str) -> Result<(usize)> {
        let mut len = self.send(line)?;
        len += self.writer
            .write(&['\n' as u8])
            .chain_err(|| "cannot write newline")?;
        self.read_line()?; // discard what we just wrote into the tty
        Ok(len)
    }


    /// sends string to process. This may be buffered. You may use `flush()` after `send()`
    /// returns number of written bytes
    ///
    /// TODO: method to send ^C, etc.
    pub fn send(&mut self, s: &str) -> Result<(usize)> {
        self.writer
            .write(s.as_bytes())
            .chain_err(|| "cannot write line to process")
    }

    /// make sure all bytes written via `send()` are sent to the process
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush().chain_err(|| "could not flush")
    }


    pub fn read_line(&mut self) -> Result<String> {
        self.reader.read_line()
    }

    pub fn exp_eof(&mut self) -> Result<()> {
        self.reader.read_until(&ReadUntil::EOF).and_then(|_| Ok(()))
    }

    pub fn exp_string(&mut self, needle:&str) -> Result<()> {
        self.reader.read_until(&ReadUntil::String(needle.to_string())).and_then(|_| {
            Ok(())
        })
    }
}

/// Start command in a pty session. Splits string at space and handles the rest as args
pub fn spawn(program: &str, timeout: Option<u64>) -> Result<PtySession> {
    let command = if program.find(" ").is_some() {
        let mut parts = program.split(" ");
        let mut cmd = Command::new(parts.next().unwrap());
        cmd.args(parts);
        cmd
    } else {
        Command::new(program)
    };
    spawn_command(command, timeout)
}

pub fn spawn_command(command: Command, timeout: Option<u64>) -> Result<PtySession> {
    let commandname = format!("{:?}", &command);
    let process = PtyProcess::new(command)
        .chain_err(|| "couldn't start process")?;
    let f = unsafe { File::from_raw_fd(process.pty.as_raw_fd()) };
    let writer = LineWriter::new(f.try_clone().chain_err(|| "couldn't open write stream")?);
    let reader = NBReader::new(f, timeout);
    Ok(PtySession {
           process: process,
           writer: writer,
           reader: reader,
           commandname: commandname,
       })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cat2() {
        || -> Result<()> {
            let mut s = spawn("cat", None)?;
            s.send_line("hans")?;
            assert_eq!("hans\r\n", s.read_line()?);
            let should = ::process::wait::WaitStatus::Signaled(s.process.child_pid,
                                                               ::process::signal::Signal::SIGTERM,
                                                               false);
            assert_eq!(should, s.process.exit()?);
            Ok(())
        }()
                .expect("could not execute");
    }


    #[test]
    fn test_timeout() {
        || -> Result<()> {
            let mut p = spawn("sleep 3", Some(1000)).expect("cannot run sleep 3");
            match p.exp_eof() {
                Ok(_) => assert!(false, "should raise Timeout"),
                Err(Error (ErrorKind::Timeout, _)) => {},
                Err(_) => assert!(false, "should raise TimeOut")

            }
            Ok(())
        }().expect("test_timeout failed");
    }

    #[test]
    fn test_timeout2() {
        let mut p = spawn("sleep 1", Some(1100)).expect("cannot run sleep 1");
        assert!(p.exp_eof().is_ok(), "expected eof");
    }

    #[test]
    fn test_cat3() {

        || -> Result<()> {
            let mut p = spawn("cat", Some(1)).expect("cannot run cat");
            p.send_line("hello world!")?;
            p.exp_string("hello world!")?;
            p.send_line("hello heaven!")?;
            p.exp_string("hello heaven!")?;
            Ok(())
        }().expect("test_cat3 failed");
    }

}
