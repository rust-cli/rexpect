extern crate rexpect;
use rexpect::spawn_bash;
use rexpect::errors::*;


fn run() -> Result<()> {
    let mut p = spawn_bash(Some(1000))?;
    p.execute("ping 8.8.8.8", "bytes of data")?;
    p.send_control('z')?;
    p.wait_for_prompt()?;
    p.send_line("bg", "continued")?;
    p.wait_for_prompt()?;
    p.send_line("sleep 1")?;
    p.wait_for_prompt()?;
    p.execute("fg", "running")?;
    p.send_control('c')?;
    p.exp_string("packet loss")?;
    Ok(())
}

fn main() {
    run().unwrap_or_else(|e| panic!("bash process failed with {}", e));
}
