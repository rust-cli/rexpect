//! Start a process via pty

use std::io;
use std::path::Path;
use std::process::Command;
use std::os::unix::process::CommandExt;
use nix::pty::{posix_openpt, grantpt, unlockpt, PtyMaster};
use nix::fcntl::{O_RDWR, open};
use nix::sys::{stat};
use nix::unistd::{fork, ForkResult, setsid, dup2};
use nix::libc::{STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO};
use errors::*; // load error-chain

#[cfg(target_os = "linux")]
use nix::pty::ptsname_r;

#[cfg(not(target_os = "linux"))]
use nix::pty::ptsname;
#[cfg(not(target_os = "linux"))]
use std::sync::Mutex;
#[cfg(not(target_os = "linux"))]
lazy_static! {
    static ref PTSNAME_MUTEX: Mutex<()> = { println!("singleton.."); Mutex::new(()) };
}

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
/// use rexpect::process::PtyProcess;
/// use std::process::Command;
/// use std::fs::File;
/// use std::io::{BufReader, LineWriter};
/// use std::os::unix::io::{FromRawFd, AsRawFd};
///
/// let process = PtyProcess::new(Command::new("cat")).expect("could not execute cat");
/// let f = unsafe { File::from_raw_fd(process.pty.as_raw_fd()) };
/// let mut writer = LineWriter::new(&f);
/// let mut reader = BufReader::new(&f);
///
/// // writer.write() sends strings to `cat`
/// // writer.reader() reads back what `cat` wrote
/// // send Ctrl-C with writer.write(&[3]) and writer.flush()
/// ```
pub struct PtyProcess {
    pub pty: PtyMaster,
    pub child_pid: i32,
}

impl PtyProcess {
    pub fn new(mut command: Command) -> Result<Self> {
        || -> io::Result<Self> {
            // Open a new PTY master
            let master_fd = posix_openpt(O_RDWR)?;

            // Allow a slave to be generated for it
            grantpt(&master_fd)?;
            unlockpt(&master_fd)?;

            // ptsname is not thread-safe. There is ptsname_r which is only available on Linux
            // -> use ptsname_r on Linux, for other OS use a mutex
            #[cfg(target_os = "linux")]
            let slave_name = ptsname_r(&master_fd)?;
            #[cfg(not(target_os = "linux"))]
            let slave_name = {
                // unwrap_or ignores poison errors
                PTSNAME_MUTEX.lock().unwrap_or_else(|error| error.into_inner())?;
                ptsname(&master_fd)?
            };

            match fork()? {
                ForkResult::Child => {
                    setsid()?; // create new session with child as session leader
                    let slave_fd = open(Path::new(&slave_name), O_RDWR, stat::Mode::empty())?;

                    // assign stdin, stdout, stderr to the tty, just like a terminal does
                    dup2(slave_fd, STDIN_FILENO)?;
                    dup2(slave_fd, STDOUT_FILENO)?;
                    dup2(slave_fd, STDERR_FILENO)?;
                    command.exec();
                    Err(io::Error::last_os_error())
                },
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
            let f = unsafe { File::from_raw_fd(process.pty.as_raw_fd()) };
            let mut writer = LineWriter::new(&f);
            let mut reader = BufReader::new(&f);
            writer.write(b"hello cat\n")?;
            let mut output = String::new();
            reader.read_line(&mut output)?; // read back what we just wrote
            reader.read_line(&mut output)?; // read back output of cat
            writer.write(&[3])?;
            writer.flush()?;

            let mut buf = [0;2];
            reader.read(&mut buf)?;
            output += &String::from_utf8_lossy(&buf).to_string();

            assert_eq!(output, "hello cat\r\n\
        hello cat\r\n\
        ^C");
            let should = wait::WaitStatus::Signaled(process.child_pid, signal::Signal::SIGINT, false);
            assert_eq!(should, wait::waitpid(process.child_pid, None)?);
            Ok(())
        }().expect("could not execute cat");
    }
}