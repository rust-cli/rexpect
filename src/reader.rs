use std::fs;
use std::io::{BufReader, self};
use std::io::prelude::*;
use std::string::FromUtf8Error;
use std::sync::mpsc::{channel, Receiver};
use std::{thread, result};
use errors::*; // load error-chain

#[derive(Debug)]
enum PipeError {
    IO(io::Error),
    NotUtf8(FromUtf8Error),
}

#[derive(Debug)]
enum PipedLine {
    Line(String),
    EOF,
}

/// Non-Blocking reader
pub struct NBReader {
    reader: Receiver<result::Result<PipedLine, PipeError>>,
}

impl NBReader {
    pub fn new(f: fs::File) -> NBReader {
        let (tx, rx) = channel();

        // spawn a thread which reads one line and sends it to tx
        thread::spawn(move || {
            let mut reader = BufReader::new(f);
            let mut buf = Vec::new();
            let mut byte = [0u8];
            loop {
                match reader.read(&mut byte) {
                    Ok(0) => {
                        let _ = tx.send(Ok(PipedLine::EOF));
                        break;
                    }
                    Ok(_) => {
                        if byte[0] == 0x0A { // \n
                            tx.send(match String::from_utf8(buf.clone()) {
                                Ok(line) => Ok(PipedLine::Line(line)),
                                Err(err) => Err(PipeError::NotUtf8(err)),
                            }).expect("cannot send to channel");
                            buf.clear();
                        } else if byte[0] == 0x0D { // \r
                            // eat \r
                        } else {
                            buf.push(byte[0]);
                        }
                    }
                    Err(error) => {
                        tx.send(Err(PipeError::IO(error))).unwrap();
                    }
                }
            };
        });
        NBReader{reader: rx}
    }

    /// read one line (blocking!), remove the line ending (because tty it is \r\n) and return it
    /// TODO: example on how to check for EOF
    pub fn read_line(&mut self) -> Result<String> {
        match self.reader.recv().chain_err(|| "cannot read from channel")? {
            Ok(PipedLine::Line(s)) => Ok(s),
            Ok(PipedLine::EOF) => Err(ErrorKind::EOF.into()),
            Err(error) => match error {
                PipeError::NotUtf8(_) => Err("got non utf8 byte".into()),
                PipeError::IO(_) => Err("got an IO error".into())
            },
        }
    }

}