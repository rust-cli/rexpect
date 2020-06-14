extern crate rexpect;

use rexpect::spawn;
use rexpect::errors::*;

/// The following code emits:
/// cat exited with code 0, all good!
/// cat exited with code 1
/// Output (stdout and stderr): cat: /this/does/not/exist: No such file or directory
fn exit_code_fun() -> Result<()> {
    let mut p = spawn("cat /etc/passwd", Some(2000))?;
    match p.process.wait() {
        Ok(status) if status.success() => println!("cat exited with code 0, all good!"),
        Ok(status) => {
            if let Some(code) = status.code() {
                println!("Cat failed with exit code {}", code);
                println!("Output (stdout and stderr): {}", p.exp_eof()?);
            } else {
                println!("cat was probably killed")
            }
        },
        Err(err) => println!("failed to wait on process {:?}", err),
    }
    
    let mut p = spawn("cat /this/does/not/exist", Some(2000))?;
    match p.process.wait() {
        Ok(status) if status.success() => println!("cat exited with code 0, all good!"),
        Ok(status) => {
            if let Some(code) = status.code() {
                println!("Cat failed with exit code {}", code);
                println!("Output (stdout and stderr): {}", p.exp_eof()?);
            } else {
                println!("cat was probably killed")
            }
        },
        Err(err) => println!("failed to wait on process {:?}", err),
    }
    
    Ok(())
}


fn main() {
    exit_code_fun().unwrap_or_else(|e| panic!("cat function failed with {}", e));
}
