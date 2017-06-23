
use std::io::{self, BufReader};
use std::io::prelude::*;
use std::sync::mpsc::{channel, Receiver};
use std::{thread, result};
use std::time;
use errors::*; // load error-chain
pub use regex::Regex;

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
    Regex(Regex),
    EOF,
    NBytes(usize),
}

/// Non blocking reader
///
/// Typically you'd need that to check for output of a process without blocking your thread.
/// Internally a thread is spawned and the output is read ahead so when
/// calling `read_line` or `read_until` it reads from an internal buffer
///
/// TODO: method to "check" for output
/// TODO: way of providing a timeout
pub struct NBReader {
    reader: Receiver<result::Result<PipedChar, PipeError>>,
    buffer: String,
    eof: bool,
    timeout: Option<time::Duration>,
}

impl NBReader {
    /// Create a new reader instance
    ///
    /// f: file like object
    /// timeout:
    /// - None: read_until is blocking forever. This is probably not what you want
    /// - Some(millis): after millis millisecons a timeout error is raised
    pub fn new<R: Read + Send + 'static>(f: R, timeout: Option<u64>) -> NBReader {
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
        NBReader {
            reader: rx,
            buffer: String::with_capacity(1024),
            eof: false,
            timeout: timeout.and_then(|millis| Some(time::Duration::from_millis(millis))),
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
                // this is just from experience, e.g. "sleep 5" returns the other error which
                // most probably means that there is no stdout stream at all
                Err(PipeError::IO ( ref err ) ) if err.kind() == io::ErrorKind::Other => return Err(ErrorKind::BrokenPipe.into()),
                // discard other errors
                Err(_) => {}
            }
        }
        Ok(())
    }

    /// read one line (blocking!) and return line including newline (\r\n for tty processes)
    /// TODO: example on how to check for EOF
    pub fn read_line(&mut self) -> Result<String> {
        return self.read_until(&ReadUntil::String('\n'.to_string()));
    }

    /// Read until needle is found (blocking!) and return string until end of needle
    ///
    /// This methods loops (while reading from the Cursor) until the needle is found.
    ///
    /// There are different modes:
    ///
    /// - `ReadUntil::String` searches for String and returns the read bytes
    ///    until and with the needle (use '\n'.to_string() to search for newline)
    /// - `ReadUntil::Regex` searches for regex and returns string until and with the found chars
    ///   which match the regex
    /// - `ReadUntil::NBytes` reads maximum n bytes
    /// - `ReadUntil::EOF` reads until end of file is reached
    ///
    /// Note that when used with a tty the lines end with \r\n
    ///
    /// Returns error if EOF is reached before the needle could be found.
    ///
    /// # Example with line reading, byte reading, regex and EOF reading.
    ///
    /// ```
    /// # use std::io::Cursor;
    /// use rexpect::reader::{NBReader, ReadUntil, Regex};
    /// // instead of a Cursor you would put your process output or file here
    /// let f = Cursor::new("Hello, miss!\n\
    ///                         What do you mean: 'miss'?");
    /// let mut e = NBReader::new(f, None);
    ///
    /// let first_line = e.read_until(&ReadUntil::String('\n'.to_string())).unwrap();
    /// assert_eq!("Hello, miss!\n", &first_line);
    ///
    /// let two_bytes = e.read_until(&ReadUntil::NBytes(2)).unwrap();
    /// assert_eq!("Wh", &two_bytes);
    /// let re = Regex::new(r"'[a-z]+'").unwrap(); // will find 'miss'
    ///
    /// let until_miss = e.read_until(&ReadUntil::Regex(re)).unwrap();
    /// assert_eq!("at do you mean: 'miss'", &until_miss);
    ///
    /// let until_end = e.read_until(&ReadUntil::EOF).unwrap();
    /// assert_eq!("?", &until_end);
    /// ```
    ///
    pub fn read_until(&mut self, needle: &ReadUntil) -> Result<String> {
        let start = time::Instant::now();
        loop {
            self.read_into_buffer()?;
            let offset = match needle {
                &ReadUntil::String(ref s) => {
                    self.buffer.find(s).and_then(|pos| Some(pos + s.len()))
                }
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
                }
                &ReadUntil::NBytes(n) => {
                    if n <= self.buffer.len() {
                        Some(n)
                    } else if self.eof && self.buffer.len() > 0 {
                        // reached almost end of buffer, return string, even though it will be
                        // smaller than the wished n bytes
                        Some(self.buffer.len())
                    } else {
                        None
                    }
                }
            };
            if let Some(offset) = offset {
                return Ok(self.buffer.drain(..offset).collect());
            } else if self.eof {
                // reached end of stream and didn't match -> error
                return Err(ErrorKind::EOF.into());
            }

            if let Some(timeout) = self.timeout {
                if start.elapsed() > timeout {
                    return Err(ErrorKind::Timeout.into());
                }
            }
            // nothing matched: wait a little
            thread::sleep(time::Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expect_melon() {
        let f = io::Cursor::new("a melon\r\n");
        let mut r = NBReader::new(f, None);
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
        let mut r = NBReader::new(f, None);
        let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
        r.read_until(&ReadUntil::Regex(re))
            .expect("regex doesn't match");
    }

    #[test]
    fn test_nbytes() {
        let f = io::Cursor::new("abcdef");
        let mut r = NBReader::new(f, None);
        assert_eq!("ab", r.read_until(&ReadUntil::NBytes(2)).expect("2 bytes"));
        assert_eq!("cde", r.read_until(&ReadUntil::NBytes(3)).expect("3 bytes"));
        assert_eq!("f", r.read_until(&ReadUntil::NBytes(4)).expect("4 bytes"));
    }
}
