//! Main module of rexpect: start new process and interact with it

use process::PtyProcess;
use reader::{NBReader, Regex};
pub use reader::ReadUntil;
use std::fs::File;
use std::io::LineWriter;
use std::process::Command;
use std::io::prelude::*;
use std::ops::{Deref, DerefMut};
use errors::*; // load error-chain
use tempfile;

/// Interact with a process with read/write/signals, etc.
#[allow(dead_code)]
pub struct PtySession {
    pub process: PtyProcess,
    pub writer: LineWriter<File>,
    pub reader: NBReader,
    pub commandname: String, // only for debugging purposes now
}

/// Start a process in a tty session, write and read from it
///
/// # Example
///
/// ```
///
/// use rexpect::spawn;
/// # use rexpect::errors::*;
///
/// # fn main() {
///     # || -> Result<()> {
/// let mut s = spawn("cat", Some(1000))?;
/// s.send_line("hello, polly!")?;
/// let line = s.read_line()?;
/// assert_eq!("hello, polly!", line);
///         # Ok(())
///     # }().expect("test failed");
/// # }
/// ```
impl PtySession {
    /// sends string and a newline to process
    ///
    /// this is guaranteed to be flushed to the process
    /// returns number of written bytes
    pub fn send_line(&mut self, line: &str) -> Result<(usize)> {
        let mut len = self.send(line)?;
        len += self.writer
            .write(&['\n' as u8])
            .chain_err(|| "cannot write newline")?;
        Ok(len)
    }


    /// Send string to process. As stdin of the process is most likely buffered, you'd
    /// need to call `flush()` after `send()` to make the process actually see your input.
    ///
    /// Returns number of written bytes
    pub fn send(&mut self, s: &str) -> Result<(usize)> {
        self.writer
            .write(s.as_bytes())
            .chain_err(|| "cannot write line to process")
    }

    /// Send a control code to the running process and consume resulting output line
    /// (which is empty because echo is off)
    ///
    /// E.g. `send_control('c')` sends ctrl-c. Upper/smaller case does not matter.
    pub fn send_control(&mut self, c: char) -> Result<()> {
        let code = match c {
            'a'...'z' => c as u8 + 1 - 'a' as u8,
            'A'...'Z' => c as u8 + 1 - 'A' as u8,
            '[' => 27,
            '\\' => 28,
            ']' => 29,
            '^' => 30,
            '_' => 31,
            _ => return Err(format!("I don't understand Ctrl-{}", c).into()),
        };
        self.writer
            .write_all(&[code])
            .chain_err(|| "cannot send control")?;
        // stdout is line buffered, so needs a flush
        self.writer
            .flush()
            .chain_err(|| "cannot flush after sending ctrl keycode")?;
        // TODO: that's completely wrong. What if the caller has not consumed
        // all strings before..? Then we would read one of the produced lines
        // earlier. A few ideas:
        // - echo on and then he can check for ^C
        // - longer wait before control and then the reader just discards the empty line
        // - we require that he reads everything before (emptying queue with try_read) and
        //   then we just retry sending the ^something until that empty line is there
        // - is ^something actually legit outside the bash environment? Shouldn't it be moved
        //   to the bash_process thingie?
        self.read_line()?;
        Ok(())
    }

    // wrapper around reader::read_until to give more context for errors
    fn exp(&mut self, needle: &ReadUntil) -> Result<(String, String)> {
        match self.reader.read_until(needle) {
            Ok(s) => Ok(s),
            Err(Error(ErrorKind::EOF(expected, got, _), _)) => {
                Err(ErrorKind::EOF(expected, got, self.process.status()).into())
            }
            Err(e) => Err(e),
        }
    }

    /// Make sure all bytes written via `send()` are sent to the process
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush().chain_err(|| "could not flush")
    }

    /// Read one line (blocking!) and return line without the newline
    /// (waits until \n is in the output fetches the line and removes \r at the end if present)
    pub fn read_line(&mut self) -> Result<String> {
        match self.exp(&ReadUntil::String('\n'.to_string())) {
            Ok((mut line, _)) => {
                if line.ends_with('\r') {
                    line.pop().expect("this never happens");
                }
                Ok(line)
            }
            Err(e) => Err(e),
        }
    }

    /// Return `Some(c)` if a char is ready in the stdout stream of the process, return `None`
    /// otherwise. This is nonblocking.
    pub fn try_read(&mut self) -> Option<char> {
        self.reader.try_read()
    }

    /// Wait until we see EOF (i.e. child process has terminated)
    /// Return all the yet unread output
    pub fn exp_eof(&mut self) -> Result<String> {
        self.exp(&ReadUntil::EOF).and_then(|(_, s)| Ok(s))
    }

    /// Wait until provided regex is seen on stdout of child process.
    /// Return a tuple:
    /// 1. the yet unread output
    /// 2. the matched regex
    pub fn exp_regex(&mut self, regex: &str) -> Result<(String, String)> {
        let res = self.exp(&ReadUntil::Regex(Regex::new(regex).chain_err(|| "invalid regex")?))
            .and_then(|s| Ok(s));
        res
    }

    /// Wait until provided string is seen on stdout of child process.
    /// Return the yet unread output (without the matched string)
    pub fn exp_string(&mut self, needle: &str) -> Result<(String)> {
        self.exp(&ReadUntil::String(needle.to_string()))
            .and_then(|(s, _)| Ok(s))
    }

    /// Wait until provided char is seen on stdout of child process.
    /// Return the yet unread output (without the matched char)
    pub fn exp_char(&mut self, needle: char) -> Result<String> {
        self.exp(&ReadUntil::String(needle.to_string()))
            .and_then(|(s, _)| Ok(s))
    }

    /// Wait until any of the provided needles is found.
    ///
    /// Return a tuple with:
    /// 1. the yet unread string, without the matching needle (empty in case of EOF and NBytes)
    /// 2. the matched string
    ///
    /// # Example:
    ///
    /// ```
    /// use rexpect::{spawn, ReadUntil};
    /// # use rexpect::errors::*;
    ///
    /// # fn main() {
    ///     # || -> Result<()> {
    /// let mut s = spawn("cat", Some(1000))?;
    /// s.send_line("hello, polly!")?;
    /// s.exp_any(vec![ReadUntil::String("hello".into()),
    ///                ReadUntil::EOF])?;
    ///         # Ok(())
    ///     # }().expect("test failed");
    /// # }
    /// ```
    pub fn exp_any(&mut self, needles: Vec<ReadUntil>) -> Result<(String, String)> {
        self.exp(&ReadUntil::Any(needles))
    }
}

/// Start command in background in a pty session (pty fork) and return a struct
/// with writer and buffered reader (for unblocking reads).
///
/// #Arguments:
///
/// - `program`: This is split at spaces and turned into a `process::Command`
///   if you wish more control over this, use `spawn_command`
/// - `timeout`: If Some: all `exp_*` commands time out after x millisecons, if None: never times
///   out.
///   It's higly recommended to put a timeout there, as otherwise in case of
///   a problem the program just hangs instead of exiting with an
///   error message indicating where it stopped.
///   For automation 30'000 (30s, the default in pexpect) is a good value.
pub fn spawn(program: &str, timeout_ms: Option<u64>) -> Result<PtySession> {
    let command = if program.find(" ").is_some() {
        let mut parts = program.split(" ");
        let mut cmd = Command::new(parts.next().unwrap());
        cmd.args(parts);
        cmd
    } else {
        Command::new(program)
    };
    spawn_command(command, timeout_ms)
}

/// See `spawn`
pub fn spawn_command(command: Command, timeout_ms: Option<u64>) -> Result<PtySession> {
    let commandname = format!("{:?}", &command);
    let mut process = PtyProcess::new(command)
        .chain_err(|| "couldn't start process")?;
    process.set_kill_timeout(timeout_ms);

    let f = process.get_file_handle();
    let writer = LineWriter::new(f.try_clone().chain_err(|| "couldn't open write stream")?);
    let reader = NBReader::new(f, timeout_ms);
    Ok(PtySession {
           process: process,
           writer: writer,
           reader: reader,
           commandname: commandname,
       })
}

/// Interact with a bash shell: execute programs, interact with ctrl-z, bg, etc.
pub struct PtyBashSession {
    prompt: String,
    pty_session: PtySession,
}

impl PtyBashSession {
    pub fn wait_for_prompt(&mut self) -> Result<String> {
        self.pty_session.exp_string(&self.prompt)
    }

    /// Send cmd to bash and wait for the ready string being present
    ///
    /// Q; Why can't I just do `send_line` and immediately continue?
    /// A: Executing a command in bash causes a fork. If the Unix kernel chooses the
    ///    parent process (bash) to go first and the bash process sends e.g. Ctrl-C then the
    ///    Ctrl-C goes to nirvana.
    ///    The only way to prevent this situation is to wait for a ready string being present
    ///    in the output.
    ///
    /// Another safe way to tackle this problem is to use `send_line()` and `wait_for_prompt()`
    ///
    /// # Example:
    ///
    /// ```
    /// use rexpect::spawn_bash;
    /// # use rexpect::errors::*;
    ///
    /// # fn main() {
    ///     # || -> Result<()> {
    /// let mut p = spawn_bash(Some(1000))?;
    /// p.execute("cat <(echo ready) -", "ready")?;
    /// p.send_line("hans")?;
    /// p.exp_string("hans")?;
    ///         # Ok(())
    ///     # }().expect("test failed");
    /// # }
    /// ```
    pub fn execute(&mut self, cmd: &str, ready_regex: &str) -> Result<()> {
        self.pty_session.send_line(cmd)?;
        self.pty_session.exp_regex(ready_regex)?;
        Ok(())
    }
}

// make PtySession's methods available directly
impl Deref for PtyBashSession {
    type Target = PtySession;
    fn deref(&self) -> &PtySession {
        &self.pty_session
    }
}

impl DerefMut for PtyBashSession {
    fn deref_mut(&mut self) -> &mut PtySession {
        &mut self.pty_session
    }
}

impl Drop for PtyBashSession {
    fn drop(&mut self) {
        // if we leave that out, PtyProcess would try to kill the bash
        // which would not work, as a SIGTERM is not enough to kill bash
        self.pty_session
            .send_line("exit")
            .expect("could not run `exit` on bash process");
    }
}


/// Spawn bash in a pty session, run programs and expect output
///
///
/// The difference to `spawn` and `spawn_command` is:
///
/// - spawn_bash starts bash with a custom rcfile which guarantees
///   a certain prompt
/// - the PtyBashSession also provides `wait_for_prompt` and `execute`
///
/// timeout: the duration until which `exp_*` returns a timeout error, or None
/// additionally, when dropping the bash prompt while bash is still blocked by a program
/// (e.g. `sleep 9999`) then the timeout is used as a timeout before a `kill -9` is issued
/// at the bash command. Use a timeout whenever possible because it makes
/// debugging a lot easier (otherwise the program just hangs and you
/// don't know where)
///
/// bash is started with echo off. That means you don't need to "read back"
/// what you wrote to bash. But what you need to do is a `wait_for_prompt`
/// after a process finished.
///
/// Also: if you start a program you should use `execute` and not `send_line`.
///
/// For an example see the README
pub fn spawn_bash(timeout: Option<u64>) -> Result<PtyBashSession> {
    // unfortunately working with a temporary tmpfile is the only
    // way to guarantee that we are "in step" with the prompt
    // all other attempts were futile, especially since we cannot
    // wait for the first prompt since we don't know what .bashrc
    // would set as PS1 and we cannot know when is the right time
    // to set the new PS1
    let mut rcfile = tempfile::NamedTempFile::new().unwrap();
    rcfile.write(b"include () { [[ -f \"$1\" ]] && source \"$1\"; }\n\
                  include /etc/bash.bashrc\n\
                  include ~/.bashrc\n\
                  PS1=\"~~~~\"\n").expect("cannot write to tmpfile");
    let mut c = Command::new("bash");
    c.args(&["--rcfile", rcfile.path().to_str().unwrap_or_else(|| return "temp file does not exist".into())]);
    spawn_command(c, timeout).and_then(|p| {
        let new_prompt = "[REXPECT_PROMPT>";
        let mut pb = PtyBashSession {
            prompt: new_prompt.to_string(),
            pty_session: p,
        };
        pb.exp_string("~~~~")?;
        rcfile.close().chain_err(|| "cannot delete temporary rcfile")?;
        pb.send_line(&("PS1='".to_string() + new_prompt + "'"))?;
        // wait until the new prompt appears
        pb.wait_for_prompt()?;
        Ok(pb)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_line() {
        || -> Result<()> {
            let mut s = spawn("cat", Some(1000))?;
            s.send_line("hans")?;
            assert_eq!("hans", s.read_line()?);
            let should = ::process::wait::WaitStatus::Signaled(s.process.child_pid,
                                                               ::process::signal::Signal::SIGTERM,
                                                               false);
            assert_eq!(should, s.process.exit()?);
            Ok(())
        }()
                .unwrap_or_else(|e| panic!("test_read_line failed: {}", e));
    }


    #[test]
    fn test_expect_eof_timeout() {
        || -> Result<()> {
            let mut p = spawn("sleep 3", Some(1000)).expect("cannot run sleep 3");
            match p.exp_eof() {
                Ok(_) => assert!(false, "should raise Timeout"),
                Err(Error(ErrorKind::Timeout(_, _, _), _)) => {}
                Err(_) => assert!(false, "should raise TimeOut"),

            }
            Ok(())
        }()
                .unwrap_or_else(|e| panic!("test_timeout failed: {}", e));
    }

    #[test]
    fn test_expect_eof_timeout2() {
        let mut p = spawn("sleep 1", Some(1100)).expect("cannot run sleep 1");
        assert!(p.exp_eof().is_ok(), "expected eof");
    }

    #[test]
    fn test_expect_string() {
        || -> Result<()> {
            let mut p = spawn("cat", Some(1000)).expect("cannot run cat");
            p.send_line("hello world!")?;
            p.exp_string("hello world!")?;
            p.send_line("hello heaven!")?;
            p.exp_string("hello heaven!")?;
            Ok(())
        }()
                .unwrap_or_else(|e| panic!("test_expect_string failed: {}", e));
    }

    #[test]
    fn test_read_string_before() {
        || -> Result<()> {
            let mut p = spawn("cat", Some(1000)).expect("cannot run cat");
            p.send_line("lorem ipsum dolor sit amet")?;
            assert_eq!("lorem ipsum dolor sit ", p.exp_string("amet")?);
            Ok(())
        }()
                .unwrap_or_else(|e| panic!("test_read_string_before failed: {}", e));
    }

    #[test]
    fn test_expect_any() {
        || -> Result<()> {
            let mut p = spawn("cat", Some(1000)).expect("cannot run cat");
            p.send_line("Hi")?;
            match p.exp_any(vec![ReadUntil::NBytes(3), ReadUntil::String("Hi".to_string())]) {
                Ok(s) => assert_eq!(("".to_string(), "Hi\r".to_string()), s),
                Err(e) => assert!(false, format!("got error: {}", e)),
            }
            Ok(())
        }()
                .unwrap_or_else(|e| panic!("test_expect_any failed: {}", e));
    }

    #[test]
    fn test_kill_timeout() {
        || -> Result<()> {
            let mut p = spawn_bash(Some(1000))?;
            p.execute("cat <(echo ready) -", "ready")?;
            Ok(())
        }().unwrap_or_else(|e| panic!("test_kill_timeout failed: {}", e));;
        // p is dropped here and kill is sent immediatly to bash
        // Since that is not enough to make bash exit, a kill -9 is sent within 1s (timeout)
    }

    #[test]
    fn test_bash() {
        || -> Result<()> {
            let mut p = spawn_bash(Some(1000))?;
            p.send_line("cd /tmp/")?;
            p.wait_for_prompt()?;
            p.send_line("pwd")?;
            assert_eq!("/tmp\r\n", p.wait_for_prompt()?);
            Ok(())
        }()
                .unwrap_or_else(|e| panic!("test_bash failed: {}", e));
    }

    #[test]
    fn test_bash_control_chars() {
        || -> Result<()> {
            let mut p = spawn_bash(Some(1000))?;
            p.execute("cat <(echo ready) -", "ready")?;
            p.send_control('c')?; // abort: SIGINT
            p.wait_for_prompt()?;
            p.execute("cat <(echo ready) -", "ready")?;
            p.send_control('z')?; // suspend:SIGTSTPcon
            p.exp_regex(r"(Stopped|suspended)\s+cat .*")?;
            p.send_line("fg")?;
            p.execute("cat <(echo ready) -", "ready")?;
            p.send_control('c')?;
            Ok(())
        }()
                .unwrap_or_else(|e| panic!("test_bash_control_chars failed: {}", e));
    }
}
