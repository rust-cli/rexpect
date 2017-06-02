use std::path::Path;
use nix::pty::{posix_openpt, grantpt, unlockpt, ptsname, PtyMaster};
use nix::fcntl::{O_RDWR, open};
use nix::sys::{stat};
use nix::unistd::{fork, ForkResult, setsid, dup2};
use nix::libc::{STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO};
use std::io::{Result, Error};
use std::process::Command;

/// Starts a process in a forked tty so you can interact with it sams as with in a terminal
///
/// # Example
///
/// Typically you want to do something like this:
///
/// ```
/// # #![allow(unused_mut)]
/// # #![allow(unused_variables)]
///
/// use rexpect::fork_pty::PtyProcess;
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
        // Open a new PTY master
        let master_fd = posix_openpt(O_RDWR)?;

        // Allow a slave to be generated for it
        grantpt(&master_fd)?;
        unlockpt(&master_fd)?;

        // Get the name of the slave
        let slave_name = ptsname(&master_fd)?;

        match fork()? {
            ForkResult::Child => {
                setsid()?; // create new session with child as session leader
                let slave_fd = open(Path::new(&slave_name), O_RDWR, stat::Mode::empty())?;

                // assign stdin, stdout, stderr to the tty, just like a terminal does
                dup2(slave_fd, STDIN_FILENO)?;
                dup2(slave_fd, STDOUT_FILENO)?;
                dup2(slave_fd, STDERR_FILENO)?;
                command.status()?;
                Err(Error::last_os_error())
            }
            ForkResult::Parent { child: child_pid } => {
                Ok(PtyProcess {
                    pty: master_fd,
                    child_pid: child_pid,
                })
            }
        }
    }
}