use ::process::PtyProcess;
use std::io::{BufReader, LineWriter, Result};
use std::ffi::OsStr;
use std::fs::File;
use std::process::Command;
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::prelude::*;

pub struct PtySession {
    process:PtyProcess,
    writer:LineWriter<File>,
    reader: BufReader<File>
}

impl PtySession {
    fn spawn<S: AsRef<OsStr>>(program: S) -> Result<Self> {
        let command = Command::new(program);
        let process = PtyProcess::new(command)?;
        let f = unsafe { File::from_raw_fd(process.pty.as_raw_fd()) };
        let mut writer = LineWriter::new(f.try_clone()?);
        let mut reader = BufReader::new(f);
        Ok(PtySession {
            process: process,
            writer: writer,
            reader: reader
        })
    }

    fn send_line(&mut self, line: &str) -> Result<()> {
        self.writer.write_all(line.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cat() {
        let s = PtySession::spawn("cat");

    }

}