//! Start a process via pty

use std;
use std::process::Command;
use std::os::unix::process::CommandExt;
use std::{thread, time};
use nix::pty::{posix_openpt, grantpt, unlockpt, PtyMaster};
use nix::fcntl::{O_RDWR, open};
use nix;
use nix::sys::stat;
use nix::unistd::{fork, ForkResult, setsid, dup2};
use nix::libc::{STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO};
pub use nix::sys::{wait, signal};
use errors::*; // load error-chain


/// Starts a process in a forked tty so you can interact with it the same as you would
/// within a terminal
///
/// The process and pty session are killed upon dropping PtyProcess
///
/// # Example
///
/// Typically you want to do something like this (for a more complete example see
/// unit test `test_cat` within this module):
///
/// ```
/// # #![allow(unused_mut)]
/// # #![allow(unused_variables)]
///
/// extern crate nix;
/// extern crate rexpect;
///
/// use rexpect::process::PtyProcess;
/// use std::process::Command;
/// use std::fs::File;
/// use std::io::{BufReader, LineWriter};
/// use std::os::unix::io::{FromRawFd, AsRawFd};
///
/// # fn main() {
///
/// let mut process = PtyProcess::new(Command::new("cat")).expect("could not execute cat");
/// let f = unsafe { File::from_raw_fd(process.pty.as_raw_fd()) };
/// let mut writer = LineWriter::new(&f);
/// let mut reader = BufReader::new(&f);
/// process.exit().expect("could not terminate process");
///
/// // writer.write() sends strings to `cat`
/// // writer.reader() reads back what `cat` wrote
/// // send Ctrl-C with writer.write(&[3]) and writer.flush()
///
/// # }
/// ```
pub struct PtyProcess {
    pub pty: PtyMaster,
    pub child_pid: i32,
}


#[cfg(target_os = "linux")]
use nix::pty::ptsname_r;

#[cfg(target_os = "macos")]
/// ptsname_r is a linux extension but ptsname isn't thread-safe
/// instead of using a static mutex this calls ioctl with TIOCPTYGNAME directly
/// based on https://blog.tarq.io/ptsname-on-osx-with-rust/
fn ptsname_r(fd: &PtyMaster) -> nix::Result<String> {
    use std::ffi::CStr;
    use std::os::unix::io::AsRawFd;
    use nix::libc::{ioctl, TIOCPTYGNAME};

    /// the buffer size on OSX is 128, defined by sys/ttycom.h
    let buf: [i8; 128] = [0; 128];

    unsafe {
        match ioctl(fd.as_raw_fd(), TIOCPTYGNAME as u64, &buf) {
            0 => {
                let res = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
                Ok(res)
            }
            _ => Err(nix::Error::last()),
        }
    }
}

impl PtyProcess {
    pub fn new(mut command: Command) -> Result<Self> {
        || -> nix::Result<Self> {
            // Open a new PTY master
            let master_fd = posix_openpt(O_RDWR)?;

            // Allow a slave to be generated for it
            grantpt(&master_fd)?;
            unlockpt(&master_fd)?;

            // on Linux this is the libc function, on OSX this is our implementation of ptsname_r
            let slave_name = ptsname_r(&master_fd)?;

            match fork()? {
                ForkResult::Child => {
                    setsid()?; // create new session with child as session leader
                    let slave_fd = open(std::path::Path::new(&slave_name),
                                        O_RDWR,
                                        stat::Mode::empty())?;

                    // assign stdin, stdout, stderr to the tty, just like a terminal does
                    dup2(slave_fd, STDIN_FILENO)?;
                    dup2(slave_fd, STDOUT_FILENO)?;
                    dup2(slave_fd, STDERR_FILENO)?;
                    command.exec();
                    Err(nix::Error::last())
                }
                ForkResult::Parent { child: child_pid } => {
                    Ok(PtyProcess {
                           pty: master_fd,
                           child_pid: child_pid,
                       })
                }
            }
        }()
                .chain_err(|| format!("could not execute {:?}", command))
    }

    /// Get status of child process, nonblocking.
    ///
    /// This method runs waitpid on the process.
    /// This means: If you ran `exit()` before or `status()` tihs method will
    /// return an Error
    ///
    /// # Example
    /// ```rust,no_run
    ///
    /// # extern crate nix;
    /// # extern crate rexpect;
    /// use rexpect::process;
    /// use std::process::Command;
    ///
    /// # fn main() {
    ///     let cmd = Command::new("/path/to/myprog");
    ///     let process = process::PtyProcess::new(cmd).expect("could not execute myprog");
    ///     while process.status().unwrap() == process::wait::WaitStatus::StillAlive {
    ///         // do something
    ///     }
    /// # }
    /// ```
    ///
    pub fn status(&self) -> Result<(wait::WaitStatus)> {
        wait::waitpid(self.child_pid, Some(wait::WNOHANG)).chain_err(|| "cannot read status")
    }

    /// Wait until process has exited. This is a blocking call.
    /// If the process doesn't terminate this will block forever.
    pub fn wait(&self) -> Result<(wait::WaitStatus)> {
        wait::waitpid(self.child_pid, None).chain_err(|| "wait: cannot read status")
    }

    /// regularly exit the process, this method is blocking until the process is dead
    pub fn exit(&mut self) -> Result<wait::WaitStatus> {
        self.kill(signal::SIGTERM)
    }

    /// kills the process with a specific signal. This method blocks, until the process is dead
    ///
    /// sends SIGTERM to the process, the pty session is closed upon dropping PtyMaster,
    /// so we don't need to explicitely do that
    ///
    /// TODO: we need a method `signal` which doesn't wait for termination
    /// TODO: this needs some way of timeout before we send a kill -9
    pub fn kill(&mut self, sig: signal::Signal) -> Result<wait::WaitStatus> {
        signal::kill(self.child_pid, sig)
            .chain_err(|| "failed to exit process")?;
        while let Ok(status) = self.status() {
            if status != wait::WaitStatus::StillAlive {
                return Ok(status);
            }
            // TODO: should support a timeout
            thread::sleep(time::Duration::from_millis(100));
        }
        Err("cannot read status, maybe it was already killed..".into())
    }
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        match self.status() {
            Ok(wait::WaitStatus::StillAlive) => {
                self.exit().expect("cannot exit");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{BufReader, LineWriter, Result};
    use nix::sys::{wait, signal};
    use std::os::unix::io::{FromRawFd, AsRawFd};
    use std::io::prelude::*;

    #[test]
    /// Open cat, write string, read back string twice, send Ctrl^C and check that cat exited
    fn test_cat() {
        // wrapping into closure so I can use ?
        || -> Result<()> {
            println!("cat: 1");
            let process = PtyProcess::new(Command::new("cat")).expect("could not execute cat");
            println!("cat: 2");
            let f = unsafe { File::from_raw_fd(process.pty.as_raw_fd()) };
            println!("cat: 3");
            let mut writer = LineWriter::new(&f);
            let mut reader = BufReader::new(&f);
            println!("cat: 4");
            writer.write(b"hello cat\n")?;
            let mut output = String::new();
            println!("cat: 5");
            reader.read_line(&mut output)?; // read back what we just wrote
            reader.read_line(&mut output)?; // read back output of cat
            println!("cat: 6");
            writer.write(&[3])?;
            writer.flush()?;

            println!("cat: 7");
            let mut buf = [0; 2];
            reader.read(&mut buf)?;
            println!("cat: 8");
            output += &String::from_utf8_lossy(&buf).to_string();

            assert_eq!(output,
                       "hello cat\r\n\
        hello cat\r\n\
        ^C");
            println!("cat: 9");
            let should =
                wait::WaitStatus::Signaled(process.child_pid, signal::Signal::SIGINT, false);
            assert_eq!(should, wait::waitpid(process.child_pid, None).unwrap());
            println!("cat: 10");
            Ok(())
        }()
                .expect("could not execute cat");
        println!("11");
    }
}
