use rexpect::fork_pty::PtyProcess;
use std::process::Command;
use std::fs::File;
use std::io::{BufReader, LineWriter, Result};
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::prelude::*;

use nix::sys::wait;
use nix::sys::signal;

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
        assert_eq!(should, wait::wait()?);
        Ok(())
    }().expect("could not execute cat");
}