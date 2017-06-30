extern crate rexpect;

use rexpect::spawn;
use rexpect::errors::*;

fn do_ssh() -> Result<()> {
    let mut p = spawn("ftp speedtest.tele2.net", Some(2000))?;
    p.exp_regex("Name \\(.*\\):")?;
    p.send_line("anonymous")?;
    p.exp_string("Password")?;
    p.send_line("test")?;
    p.exp_string("ftp>")?;
    p.send_line("cd upload")?;
    p.exp_string("successfully changed.\r\nftp>")?;
    p.send_line("pwd")?;
    p.exp_regex("[0-9]+ \"/upload\"")?;
    Ok(())
}


fn main() {
    match do_ssh() {
        Err(e) => println!("ran into problem: {}", e),
        Ok(_) => {}
    }
}
