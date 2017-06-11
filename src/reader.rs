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
    buffer: String
}

impl NBReader {
    pub fn new(f: fs::File) -> NBReader {
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
        NBReader{reader: rx, buffer: String::with_capacity(1024)}
    }

    /// reads all available chars from the read channel and stores them in self.buffer
    fn read_into_buffer(&mut self) -> Result<()> {
        while let Ok(from_channel) = self.reader.try_recv() {
            match from_channel {
                Ok(PipedChar::Char(c)) => self.buffer.push(c as char),
                Ok(PipedChar::EOF) => return Err(ErrorKind::EOF.into()),
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
                return Ok((&self.buffer[0..pos + 1]).to_string());
            }
        }
    }

}