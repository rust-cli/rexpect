//! Start a process via pty

use std;
use std::process::Command;
use std::os::unix::process::CommandExt;
use pty::{posix_openpt, grantpt, unlockpt, PtyMaster};
use nix::fcntl::{O_RDWR, open};
use nix;
use nix::sys::stat;
use nix::unistd::{fork, ForkResult, setsid, dup2};
use nix::libc::{STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO};
use errors::*; // load error-chain


/// Starts a process in a forked tty so you can interact with it sams as with in a terminal
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
/// use nix::sys::signal;
///
/// # fn main() {
///
/// let process = PtyProcess::new(Command::new("cat")).expect("could not execute cat");
/// let f = unsafe { File::from_raw_fd(process.pty.as_raw_fd()) };
/// let mut writer = LineWriter::new(&f);
/// let mut reader = BufReader::new(&f);
/// signal::kill(process.child_pid, signal::SIGTERM).expect("could not terminate process");
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
use pty::ptsname_r;

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
		println!("returning: res={}", res);
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

            let slave_name = ptsname_r(&master_fd)?;
            println!("ptsname: {}, master_fd: {:?}, command: {:?} <=================", slave_name, master_fd, command);

            match fork()? {
                ForkResult::Child => {
                    setsid()?; // create new session with child as session leader
                    let slave_fd = open(std::path::Path::new(&slave_name), O_RDWR, stat::Mode::empty())?;

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
        }().chain_err(|| format!("could not execute {:?}", command))
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
            let process = PtyProcess::new(Command::new("cat")).expect("could not execute cat");
            println!("test_cat: pid: {}", process.child_pid);
            let f = unsafe { File::from_raw_fd(process.pty.as_raw_fd()) };
            let mut writer = LineWriter::new(&f);
            let mut reader = BufReader::new(&f);
            writer.write(b"hello cat\n")?;
            let mut output = String::new();
            reader.read_line(&mut output)?; // read back what we just wrote
            reader.read_line(&mut output)?; // read back output of cat
            writer.write(&[3])?;
            writer.flush()?;

            let mut buf = [0; 2];
            reader.read(&mut buf)?;
            output += &String::from_utf8_lossy(&buf).to_string();

            assert_eq!(output,
                       "hello cat\r\n\
        hello cat\r\n\
        ^C");
            let should =
                wait::WaitStatus::Signaled(process.child_pid, signal::Signal::SIGINT, false);
            assert_eq!(should, wait::waitpid(process.child_pid, None).unwrap());
            Ok(())
        }()
                .expect("could not execute cat");
    }
}
