//! An example how you would test your own repl

extern crate rexpect;

use rexpect::spawn;
use rexpect::session::PtyReplSession;
use rexpect::errors::*;

fn ed_session() -> Result<PtyReplSession> {
    let mut ed = PtyReplSession {
        // for `echo_on` you need to figure that out by trial and error.
        // For bash and python repl it is false
        echo_on: false,

        // used for `wait_for_prompt()`
        prompt: "> ".to_string(),
        pty_session: spawn("/bin/ed -p '> '", Some(2000))?,
        // command which is sent when the instance of this struct is dropped
        // in the below example this is not needed, but if you don't explicitly
        // exit a REPL then rexpect tries to send a SIGTERM and depending on the repl
        // this does not end the repl and would end up in an error
        quit_command: Some("Q".to_string()),
    };
    ed.wait_for_prompt()?;
    Ok(ed)
}

fn do_ed_repl() -> Result<()> {
    let mut ed = ed_session()?;
    ed.send_line("a")?;
    ed.send_line("ed is the best editor evar")?;
    ed.send_line(".")?;
    ed.wait_for_prompt()?;
    ed.send_line(",l")?;
    ed.exp_string("ed is the best editor evar$")?;
    ed.send_line("Q")?;
    ed.exp_eof()?;
    Ok(())
}

fn main() {
    do_ed_repl().unwrap_or_else(|e| panic!("ed session failed with {}", e));
}