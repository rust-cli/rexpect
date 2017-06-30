extern crate rexpect;

use rexpect::spawn;
use rexpect::errors::*;

fn do_ftp() -> Result<()> {
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
    p.send_line("exit")?;
    p.exp_eof()?;
    Ok(())
}


fn main() {
    do_ftp().unwrap_or_else(|e| panic!("ftp job failed with {}", e));
}
