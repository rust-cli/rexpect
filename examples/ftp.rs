use rexpect::error::Error;
use rexpect::spawn;
use std::time;

fn main() -> Result<(), Error> {
    let mut p = spawn(
        "ftp speedtest.tele2.net",
        Some(time::Duration::from_secs(2)),
    )?;
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
