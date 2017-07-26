extern crate rexpect;
use rexpect::spawn_bash;
use rexpect::errors::*;


fn run() -> Result<()> {
    let mut p = spawn_bash(None)?;
    p.execute("ping 8.8.8.8")?;
    p.send_control('z')?;
    p.wait_for_prompt()?;
    p.execute("bg")?;
    p.wait_for_prompt()?;
    p.execute("sleep 1")?;
    p.wait_for_prompt()?;
    p.execute("fg")?;
    p.send_control('c')?;
    p.exp_string("packet loss")?;
    Ok(())
}

fn main() {
    run().unwrap_or_else(|e| panic!("bash process failed with {}", e));
}
