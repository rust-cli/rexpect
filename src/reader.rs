//! Unblocking reader which supports waiting for strings/regexes and EOF to be present

use std::io::{self, BufReader};
use std::io::prelude::*;
use std::sync::mpsc::{channel, Receiver};
use std::{thread, result};
use std::{time, fmt};
use crate::errors::*; // load error-chain
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
    Any(Vec<ReadUntil>),
}

impl fmt::Display for ReadUntil {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            &ReadUntil::String(ref s) if s == "\n" => "\\n (newline)".to_string(),
            &ReadUntil::String(ref s) if s == "\r" => "\\r (carriage return)".to_string(),
            &ReadUntil::String(ref s) => format!("\"{}\"", s),
            &ReadUntil::Regex(ref r) => format!("Regex: \"{}\"", r),
            &ReadUntil::EOF => "EOF (End of File)".to_string(),
            &ReadUntil::NBytes(n) => format!("reading {} bytes", n),
            &ReadUntil::Any(ref v) => {
                let mut res = Vec::new();
                for r in v {
                    res.push(r.to_string());
                }
                res.join(", ")
            }
        };
        write!(f, "{}", printable)
    }
}

/// find first occurrence of needle within buffer
///
/// # Arguments:
///
/// - buffer: the currently read buffer from a process which will still grow in the future
/// - eof: if the process already sent an EOF or a HUP
///
/// # Return
///
/// Tuple with match positions:
/// 1. position before match (0 in case of EOF and Nbytes)
/// 2. position after match
pub fn find(needle: &ReadUntil, buffer: &str, eof: bool) -> Option<(usize, usize)> {
    match needle {
        &ReadUntil::String(ref s) => buffer.find(s).and_then(|pos| Some((pos, pos + s.len()))),
        &ReadUntil::Regex(ref pattern) => {
            if let Some(mat) = pattern.find(buffer) {
                Some((mat.start(), mat.end()))
            } else {
                None
            }
        }
        &ReadUntil::EOF => if eof { Some((0, buffer.len())) } else { None },
        &ReadUntil::NBytes(n) => {
            if n <= buffer.len() {
                Some((0, n))
            } else if eof && buffer.len() > 0 {
                // reached almost end of buffer, return string, even though it will be
                // smaller than the wished n bytes
                Some((0, buffer.len()))
            } else {
                None
            }
        }
        &ReadUntil::Any(ref any) => {
            for read_until in any {
                if let Some(pos_tuple) = find(&read_until, buffer, eof) {
                    return Some(pos_tuple);
                }
            }
            None
        }
    }
}

/// Non blocking reader
///
/// Typically you'd need that to check for output of a process without blocking your thread.
/// Internally a thread is spawned and the output is read ahead so when
/// calling `read_line` or `read_until` it reads from an internal buffer
pub struct NBReader {
    reader: Receiver<result::Result<PipedChar, PipeError>>,
    buffer: String,
    eof: bool,
    timeout: Option<time::Duration>,
}

impl NBReader {
    /// Create a new reader instance
    ///
    /// # Arguments:
    ///
    /// - f: file like object
    /// - timeout:
    ///  + `None`: read_until is blocking forever. This is probably not what you want
    ///  + `Some(millis)`: after millis millisecons a timeout error is raised
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
                // most probably means that there is no stdout stream at all -> send EOF
                // this only happens on Linux, not on OSX
                Err(PipeError::IO(ref err)) if err.kind() == io::ErrorKind::Other => {
                    self.eof = true
                }
                // discard other errors
                Err(_) => {}
            }
        }
        Ok(())
    }

    /// Read until needle is found (blocking!) and return tuple with:
    /// 1. yet unread string until and without needle
    /// 2. matched needle
    ///
    /// This methods loops (while reading from the Cursor) until the needle is found.
    ///
    /// There are different modes:
    ///
    /// - `ReadUntil::String` searches for string (use '\n'.to_string() to search for newline).
    ///   Returns not yet read data in first String, and needle in second String
    /// - `ReadUntil::Regex` searches for regex
    ///   Returns not yet read data in first String and matched regex in second String
    /// - `ReadUntil::NBytes` reads maximum n bytes
    ///   Returns n bytes in second String, first String is left empty
    /// - `ReadUntil::EOF` reads until end of file is reached
    ///   Returns all bytes in second String, first is left empty
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
    /// let (first_line, _) = e.read_until(&ReadUntil::String('\n'.to_string())).unwrap();
    /// assert_eq!("Hello, miss!", &first_line);
    ///
    /// let (_, two_bytes) = e.read_until(&ReadUntil::NBytes(2)).unwrap();
    /// assert_eq!("Wh", &two_bytes);
    ///
    /// let re = Regex::new(r"'[a-z]+'").unwrap(); // will find 'miss'
    /// let (before, reg_match) = e.read_until(&ReadUntil::Regex(re)).unwrap();
    /// assert_eq!("at do you mean: ", &before);
    /// assert_eq!("'miss'", &reg_match);
    ///
    /// let (_, until_end) = e.read_until(&ReadUntil::EOF).unwrap();
    /// assert_eq!("?", &until_end);
    /// ```
    ///
    pub fn read_until(&mut self, needle: &ReadUntil) -> Result<(String, String)> {
        let start = time::Instant::now();

        loop {
            self.read_into_buffer()?;
            if let Some(tuple_pos) = find(needle, &self.buffer, self.eof) {
                let first = self.buffer.drain(..tuple_pos.0).collect();
                let second = self.buffer.drain(..tuple_pos.1 - tuple_pos.0).collect();
                return Ok((first, second));
            }

            // reached end of stream and didn't match -> error
            // we don't know the reason of eof yet, so we provide an empty string
            // this will be filled out in session::exp()
            if self.eof {
                return Err(ErrorKind::EOF(needle.to_string(), self.buffer.clone(), None).into());
            }

            // ran into timeout
            if let Some(timeout) = self.timeout {
                if start.elapsed() > timeout {
                    return Err(ErrorKind::Timeout(needle.to_string(),
                                                  self.buffer.clone()
                                                      .replace("\n", "`\\n`\n")
                                                      .replace("\r", "`\\r`")
                                                      .replace('\u{1b}', "`^`"),
                                                  timeout)
                                       .into());
                }
            }
            // nothing matched: wait a little
            thread::sleep(time::Duration::from_millis(100));
        }
    }

    /// Try to read one char from internal buffer. Returns None if
    /// no char is ready, Some(char) otherwise. This is nonblocking
    pub fn try_read(&mut self) -> Option<char> {
        // discard eventual errors, EOF will be handled in read_until correctly
        let _ = self.read_into_buffer();
        if self.buffer.len() > 0 {
            self.buffer.drain(..1).last()
        } else {
            None
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
        assert_eq!(("a melon".to_string(), "\r\n".to_string()),
                   r.read_until(&ReadUntil::String("\r\n".to_string()))
                       .expect("cannot read line"));
        // check for EOF
        match r.read_until(&ReadUntil::NBytes(10)) {
            Ok(_) => assert!(false),
            Err(Error(ErrorKind::EOF(_, _, _), _)) => {}
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
    fn test_regex2() {
        let f = io::Cursor::new("2014-03-15");
        let mut r = NBReader::new(f, None);
        let re = Regex::new(r"-\d{2}-").unwrap();
        assert_eq!(("2014".to_string(), "-03-".to_string()),
                   r.read_until(&ReadUntil::Regex(re))
                       .expect("regex doesn't match"));
    }

    #[test]
    fn test_nbytes() {
        let f = io::Cursor::new("abcdef");
        let mut r = NBReader::new(f, None);
        assert_eq!(("".to_string(), "ab".to_string()),
                   r.read_until(&ReadUntil::NBytes(2)).expect("2 bytes"));
        assert_eq!(("".to_string(), "cde".to_string()),
                   r.read_until(&ReadUntil::NBytes(3)).expect("3 bytes"));
        assert_eq!(("".to_string(), "f".to_string()),
                   r.read_until(&ReadUntil::NBytes(4)).expect("4 bytes"));
    }

    #[test]
    fn test_eof() {
        let f = io::Cursor::new("lorem ipsum dolor sit amet");
        let mut r = NBReader::new(f, None);
        r.read_until(&ReadUntil::NBytes(2)).expect("2 bytes");
        assert_eq!(("".to_string(), "rem ipsum dolor sit amet".to_string()),
                   r.read_until(&ReadUntil::EOF).expect("reading until EOF"));
    }

    #[test]
    fn test_try_read() {
        let f = io::Cursor::new("lorem");
        let mut r = NBReader::new(f, None);
        r.read_until(&ReadUntil::NBytes(4)).expect("4 bytes");
        assert_eq!(Some('m'), r.try_read());
        assert_eq!(None, r.try_read());
        assert_eq!(None, r.try_read());
        assert_eq!(None, r.try_read());
        assert_eq!(None, r.try_read());
    }

}
