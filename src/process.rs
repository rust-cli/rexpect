//! Start a process via pty

use std;
use std::fs::File;
use std::process::Command;
use std::os::unix::process::CommandExt;
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::{thread, time};
use nix::pty::{posix_openpt, grantpt, unlockpt, PtyMaster};
use nix::fcntl::{OFlag, open};
use nix;
use nix::sys::{stat, termios};
use nix::unistd::{fork, ForkResult, setsid, dup, dup2, Pid};
use nix::libc::{STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO};
pub use nix::sys::{wait, signal};
use crate::errors::*; // load error-chain

/// Start a process in a forked tty so you can interact with it the same as you would
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
/// use nix::unistd::dup;
///
/// # fn main() {
///
/// let mut process = PtyProcess::new(Command::new("cat")).expect("could not execute cat");
/// let fd = dup(process.pty.as_raw_fd()).unwrap();
/// let f = unsafe { File::from_raw_fd(fd) };
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
    pub child_pid: Pid,
    kill_timeout: Option<time::Duration>,
}


#[cfg(target_os = "linux")]
use nix::pty::ptsname_r;

#[cfg(target_os = "macos")]
/// ptsname_r is a linux extension but ptsname isn't thread-safe
/// instead of using a static mutex this calls ioctl with TIOCPTYGNAME directly
/// based on https://blog.tarq.io/ptsname-on-osx-with-rust/
fn ptsname_r(fd: &PtyMaster) -> nix::Result<String> {
    use std::ffi::CStr;
    use nix::libc::{ioctl, TIOCPTYGNAME};

    // the buffer size on OSX is 128, defined by sys/ttycom.h
    let mut buf: [i8; 128] = [0; 128];

    unsafe {
        match ioctl(fd.as_raw_fd(), TIOCPTYGNAME as u64, &mut buf) {
            0 => {
                let res = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
                Ok(res)
            }
            _ => Err(nix::Error::last()),
        }
    }
}

impl PtyProcess {
    /// Start a process in a forked pty
    pub fn new(mut command: Command) -> Result<Self> {
        || -> nix::Result<Self> {
            // Open a new PTY master
            let master_fd = posix_openpt(OFlag::O_RDWR)?;

            // Allow a slave to be generated for it
            grantpt(&master_fd)?;
            unlockpt(&master_fd)?;

            // on Linux this is the libc function, on OSX this is our implementation of ptsname_r
            let slave_name = ptsname_r(&master_fd)?;

            match fork()? {
                ForkResult::Child => {
                    setsid()?; // create new session with child as session leader
                    let slave_fd = open(std::path::Path::new(&slave_name),
                                        OFlag::O_RDWR,
                                        stat::Mode::empty())?;

                    // assign stdin, stdout, stderr to the tty, just like a terminal does
                    dup2(slave_fd, STDIN_FILENO)?;
                    dup2(slave_fd, STDOUT_FILENO)?;
                    dup2(slave_fd, STDERR_FILENO)?;

                    // set echo off
                    let mut flags = termios::tcgetattr(STDIN_FILENO)?;
                    flags.local_flags &= !termios::LocalFlags::ECHO;
                    termios::tcsetattr(STDIN_FILENO, termios::SetArg::TCSANOW, &flags)?;

                    command.exec();
                    Err(nix::Error::last())
                }
                ForkResult::Parent { child: child_pid } => {
                    Ok(PtyProcess {
                           pty: master_fd,
                           child_pid: child_pid,
                           kill_timeout: None,
                       })
                }
            }
        }()
                .chain_err(|| format!("could not execute {:?}", command))
    }

    /// Get handle to pty fork for reading/writing
    pub fn get_file_handle(&self) -> File {
        // needed because otherwise fd is closed both by dropping process and reader/writer
        let fd = dup(self.pty.as_raw_fd()).unwrap();
        unsafe { File::from_raw_fd(fd) }
    }

    /// At the drop of PtyProcess the running process is killed. This is blocking forever if
    /// the process does not react to a normal kill. If kill_timeout is set the process is
    /// `kill -9`ed after duration
    pub fn set_kill_timeout(&mut self, timeout_ms: Option<u64>) {
        self.kill_timeout = timeout_ms.and_then(|millis| Some(time::Duration::from_millis(millis)));
    }

    /// Get status of child process, nonblocking.
    ///
    /// This method runs waitpid on the process.
    /// This means: If you ran `exit()` before or `status()` this method will
    /// return `None`
    ///
    /// # Example
    /// ```rust,no_run
    ///
    /// # extern crate rexpect;
    /// use rexpect::process::{self, wait::WaitStatus};
    /// use std::process::Command;
    ///
    /// # fn main() {
    /// let cmd = Command::new("/path/to/myprog");
    /// let process = process::PtyProcess::new(cmd).expect("could not execute myprog");
    /// while let Some(WaitStatus::StillAlive) = process.status() {
    ///     // do something
    /// }
    /// # }
    /// ```
    ///
    pub fn status(&self) -> Option<wait::WaitStatus> {
        if let Ok(status) = wait::waitpid(self.child_pid, Some(wait::WaitPidFlag::WNOHANG)) {
            Some(status)
        } else {
            None
        }
    }

    /// Wait until process has exited. This is a blocking call.
    /// If the process doesn't terminate this will block forever.
    pub fn wait(&self) -> Result<wait::WaitStatus> {
        wait::waitpid(self.child_pid, None).chain_err(|| "wait: cannot read status")
    }

    /// Regularly exit the process, this method is blocking until the process is dead
    pub fn exit(&mut self) -> Result<wait::WaitStatus> {
        self.kill(signal::SIGTERM)
    }

    /// Nonblocking variant of `kill()` (doesn't wait for process to be killed)
    pub fn signal(&mut self, sig: signal::Signal) -> Result<()> {
        signal::kill(self.child_pid, sig)
            .chain_err(|| "failed to send signal to process")?;
        Ok(())
    }

    /// Kill the process with a specific signal. This method blocks, until the process is dead
    ///
    /// repeatedly sends SIGTERM to the process until it died,
    /// the pty session is closed upon dropping PtyMaster,
    /// so we don't need to explicitely do that here.
    ///
    /// if `kill_timeout` is set and a repeated sending of signal does not result in the process
    /// being killed, then `kill -9` is sent after the `kill_timeout` duration has elapsed.
    pub fn kill(&mut self, sig: signal::Signal) -> Result<wait::WaitStatus> {
        let start = time::Instant::now();
        loop {
            match signal::kill(self.child_pid, sig) {
                Ok(_) => {}
                // process was already killed before -> ignore
                Err(nix::Error::Sys(nix::errno::Errno::ESRCH)) => {
                    return Ok(wait::WaitStatus::Exited(Pid::from_raw(0), 0))
                }
                Err(e) => return Err(format!("kill resulted in error: {:?}", e).into()),
            }


            match self.status() {
                Some(status) if status != wait::WaitStatus::StillAlive => return Ok(status),
                Some(_) | None => thread::sleep(time::Duration::from_millis(100)),
            }
            // kill -9 if timout is reached
            if let Some(timeout) = self.kill_timeout {
                if start.elapsed() > timeout {
                    signal::kill(self.child_pid, signal::Signal::SIGKILL).chain_err(|| "")?
                }
            }
        }
    }
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        match self.status() {
            Some(wait::WaitStatus::StillAlive) => {
                self.exit().expect("cannot exit");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, LineWriter};
    use nix::sys::{wait, signal};
    use std::io::prelude::*;

    #[test]
    /// Open cat, write string, read back string twice, send Ctrl^C and check that cat exited
    fn test_cat() {
        // wrapping into closure so I can use ?
        || -> std::io::Result<()> {
            let process = PtyProcess::new(Command::new("cat")).expect("could not execute cat");
            let f = process.get_file_handle();
            let mut writer = LineWriter::new(&f);
            let mut reader = BufReader::new(&f);
            writer.write(b"hello cat\n")?;
            let mut buf = String::new();
            reader.read_line(&mut buf)?;
            assert_eq!(buf, "hello cat\r\n");

            // this sleep solves an edge case of some cases when cat is somehow not "ready"
            // to take the ^C (occasional test hangs)
            thread::sleep(time::Duration::from_millis(100));
            writer.write_all(&[3])?; // send ^C
            writer.flush()?;
            let should =
                wait::WaitStatus::Signaled(process.child_pid, signal::Signal::SIGINT, false);
            assert_eq!(should, wait::waitpid(process.child_pid, None).unwrap());
            Ok(())
        }()
                .unwrap_or_else(|e| panic!("test_cat failed: {}", e));
    }
}
