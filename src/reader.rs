use std::fs;
use std::io::{BufReader, self};
use std::io::prelude::*;
use std::sync::mpsc::{channel, Receiver};
use std::{thread, result};
use errors::*; // load error-chain

#[derive(Debug)]
enum PipeError {
    IO(io::Error),
}

#[derive(Debug)]
enum PipedChar {
    Char(u8),
    EOF,
}

/// Non-Blocking reader
pub struct NBReader {
    reader: Receiver<result::Result<PipedChar, PipeError>>,
    buffer: String,
    eof: bool
}

impl NBReader {
    pub fn new<R:Read+Send+ 'static>(f: R) -> NBReader {
        let (tx, rx) = channel();

        // spawn a thread which reads one char and sends it to tx
        thread::spawn(move || {
            let mut reader = BufReader::new(f);
            let mut byte = [0u8];
            loop {
                match reader.read(&mut byte) {
                    Ok(0) => {
                        let _ = tx.send(Ok(PipedChar::EOF));
                        break;
                    }
                    Ok(_) => {
                        tx.send(Ok(PipedChar::Char(byte[0]))).expect("cannot send char");
                    }
                    Err(error) => {
                        tx.send(Err(PipeError::IO(error))).expect("cannot send error");
                    }
                }
            };
        });
        // allocate string with a initial capacity of 1024, so when appending chars
        // we don't need to reallocate memory often
        NBReader{reader: rx, buffer: String::with_capacity(1024), eof: false}
    }

    /// reads all available chars from the read channel and stores them in self.buffer
    fn read_into_buffer(&mut self) -> Result<()> {
        if self.eof {
            return Ok(())
        }
        while let Ok(from_channel) = self.reader.try_recv() {
            match from_channel {
                Ok(PipedChar::Char(c)) => self.buffer.push(c as char),
                Ok(PipedChar::EOF) => self.eof = true,
                Err(_) => return Err("cannot read from channel".into())
            }
        }
        Ok(())
    }

    /// read one line (blocking!) and return line including newline (\r\n for tty processes)
    /// TODO: example on how to check for EOF
    pub fn read_line(&mut self) -> Result<String> {
        loop {
            self.read_into_buffer()?;
            if let Some(pos) = self.buffer.find('\n') {
                return Ok(self.buffer.drain(..pos + 1).collect())
            }
        }
    }

    pub fn expect(&mut self, needle: &str) -> Result<()> {
        loop {
            self.read_into_buffer()?;
            if let Some(pos) = self.buffer.find(needle) {
                self.buffer.drain(..pos + 1);
                return Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expect_string() {
        let f = io::Cursor::new("hans\r\n");
        let mut r = NBReader::new(f);
        assert_eq!("hans\r\n", r.read_line().expect("cannot read line"));
    }
}