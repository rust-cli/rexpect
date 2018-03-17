extern crate rexpect;

use rexpect::spawn_python;
use rexpect::errors::*;

fn do_python_repl() -> Result<()> {
    let mut p = spawn_python(Some(2000))?;
    p.send_line("1+1")?;
    p.exp_string("2")?;
    p.wait_for_prompt()?;
    Ok(())
}

fn main() {
    do_python_repl().unwrap_or_else(|e| panic!("python job failed with {}", e));
}
