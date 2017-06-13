use std::io::{BufReader, self};
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

pub enum MatchMethod {
    FindString(String),
    FindRegex(regex::Regex),
    FindEOF,
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
                            tx.send(Ok(PipedChar::Char(byte[0]))).chain_err(|| "cannot send")?;
                        }
                        Err(error) => {
                            tx.send(Err(PipeError::IO(error))).chain_err(|| "cannot send")?;
                        }
                    }
                };
                Ok(())
            }();
            // don't do error handling as on an error it was most probably the main thread which exited
            // (remote hangup)
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
            if self.eof {
                return Err(ErrorKind::EOF.into());
            }
            self.read_into_buffer()?;
            if let Some(pos) = self.buffer.find('\n') {
                return Ok(self.buffer.drain(..pos + 1).collect())
            }
        }
    }

    pub fn expect(&mut self, needle: &MatchMethod) -> Result<()> {
        use self::MatchMethod::*;
        loop {
            if self.eof {
                if let &FindEOF = needle {
                    return Ok(());
                }
                return Err(ErrorKind::EOF.into());
            }
            self.read_into_buffer()?;
            let pos = match needle {
                &FindString(ref s) => {
                    self.buffer.find(s)
                },
                &FindRegex(ref r) => {
                    println!("regex..");
                    if let Some(mat) = r.find(&self.buffer) {
                        Some(mat.end())
                    } else {
                        println!("no match this time..");
                        None
                    }
                },
                &FindEOF => {
                    None
                }
            };
            if let Some(pos) = pos {
                if pos == self.buffer.len() {
                    self.buffer.drain(..);
                } else {
                    self.buffer.drain(..pos + 1);
                }
                return Ok(())
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
        let mut r = NBReader::new(f);
        assert_eq!("a melon\r\n", r.read_line().expect("cannot read line"));
        // check for EOF
        match r.read_line() {
            Ok(_) => assert!(false),
            Err(Error(ErrorKind::EOF, _)) => {} ,
            Err(Error(_, _)) => {assert!(false)},
        }
    }

    #[test]
    fn test_regex() {
        let f = io::Cursor::new("2014-03-15");
        let mut r = NBReader::new(f);
        let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
        r.expect(&MatchMethod::FindRegex(re)).expect("regex doesn't match");
    }

}