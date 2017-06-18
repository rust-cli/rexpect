use std::io::{self, BufReader};
use std::io::prelude::*;
use std::sync::mpsc::{channel, Receiver};
use std::{thread, result};
use errors::*; // load error-chain
use regex;

#[derive(Debug)]
enum PipeError {
    IO(io::Error),
}

#[derive(Debug)]
enum PipedChar {
    Char(u8),
    EOF,
}

pub enum ReadUntil {
    String(String),
    Regex(regex::Regex),
    EOF,
    NBytes(usize),
}

/// Non blocking reader
///
/// Typically you'd need that to check for output of a process without blocking your thread.
/// Internally a thread is spawned and the output is read ahead so when
/// calling `read_line` or `expect` it reads from an internal buffer
///
///
pub struct Expecter {
    reader: Receiver<result::Result<PipedChar, PipeError>>,
    buffer: String,
    eof: bool,
}

impl Expecter {
    pub fn new<R: Read + Send + 'static>(f: R) -> Expecter {
        let (tx, rx) = channel();

        // spawn a thread which reads one char and sends it to tx
        thread::spawn(move || {
            let _ = || -> Result<()> {
                let mut reader = BufReader::new(f);
                let mut byte = [0u8];
                loop {
                    match reader.read(&mut byte) {
                        Ok(0) => {
                            let _ = tx.send(Ok(PipedChar::EOF)).chain_err(|| "cannot send")?;
                            break;
                        }
                        Ok(_) => {
                            tx.send(Ok(PipedChar::Char(byte[0])))
                                .chain_err(|| "cannot send")?;
                        }
                        Err(error) => {
                            tx.send(Err(PipeError::IO(error)))
                                .chain_err(|| "cannot send")?;
                        }
                    }
                }
                Ok(())
            }();
            // don't do error handling as on an error it was most probably
            // the main thread which exited (remote hangup)
        });
        // allocate string with a initial capacity of 1024, so when appending chars
        // we don't need to reallocate memory often
        Expecter {
            reader: rx,
            buffer: String::with_capacity(1024),
            eof: false,
        }
    }

    /// reads all available chars from the read channel and stores them in self.buffer
    fn read_into_buffer(&mut self) -> Result<()> {
        if self.eof {
            return Ok(());
        }
        while let Ok(from_channel) = self.reader.try_recv() {
            match from_channel {
                Ok(PipedChar::Char(c)) => self.buffer.push(c as char),
                Ok(PipedChar::EOF) => self.eof = true,
                Err(_) => return Err("cannot read from channel".into()),
            }
        }
        Ok(())
    }

    /// read one line (blocking!) and return line including newline (\r\n for tty processes)
    /// TODO: example on how to check for EOF
    pub fn read_line(&mut self) -> Result<String> {
        return self.expect(&ReadUntil::String('\n'.to_string()))
    }

    /// Read until needle is found (blocking!) and return string until needle
    ///
    /// # Example
    ///
    /// ```
    /// # use std::io::Cursor;
    /// use rexpect::reader::{Expecter, ReadUntil};
    /// // instead of a Cursor you would put your process output or file here
    /// let f = Cursor::new("Hello, miss!\n\
    ///                         What do you mean: 'miss'?\n\
    ///                         Oh, sorry, I have a cold");
    /// let mut e = Expecter::new(f);
    /// let first_line = e.expect(&ReadUntil::String('\n'.to_string())).unwrap();
    /// assert_eq!("Hello, miss!\n", &first_line);
    /// let two_bytes = e.expect(&ReadUntil::NBytes(2)).unwrap();
    /// assert_eq!("Wh", &two_bytes);
    /// ```
    ///
    pub fn expect(&mut self, needle: &ReadUntil) -> Result<String> {
        loop {
            self.read_into_buffer()?;
            let pos = match needle {
                &ReadUntil::String(ref s) => self.buffer.find(s),
                &ReadUntil::Regex(ref r) => {
                    if let Some(mat) = r.find(&self.buffer) {
                        Some(mat.end())
                    } else {
                        None
                    }
                }
                &ReadUntil::EOF => {
                    if self.eof {
                        Some(self.buffer.len())
                    } else {
                        None
                    }
                },
                &ReadUntil::NBytes(n) => {
                    if n <= self.buffer.len() {
                        Some(n)
                    } else {
                        None
                    }
                }
            };
            if let Some(pos) = pos {
                let ret = if pos == self.buffer.len() {
                    self.buffer.drain(..).collect()
                } else {
                    self.buffer.drain(..pos + 1).collect()
                };
                return Ok(ret);
            } else if self.eof {
                // reached end of stream and didn't match -> error
                return Err(ErrorKind::EOF.into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expect_melon() {
        let f = io::Cursor::new("a melon\r\n");
        let mut r = Expecter::new(f);
        assert_eq!("a melon\r\n", r.read_line().expect("cannot read line"));
        // check for EOF
        match r.read_line() {
            Ok(_) => assert!(false),
            Err(Error(ErrorKind::EOF, _)) => {}
            Err(Error(_, _)) => assert!(false),
        }
    }

    #[test]
    fn test_regex() {
        let f = io::Cursor::new("2014-03-15");
        let mut r = Expecter::new(f);
        let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
        r.expect(&ReadUntil::Regex(re))
            .expect("regex doesn't match");
    }

}
