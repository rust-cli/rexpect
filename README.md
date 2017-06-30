[![Build Status](https://api.travis-ci.org/philippkeller/rexpect.svg?branch=master)](https://travis-ci.org/philippkeller/rexpect)

[Documentation (Development)](http://philippkeller.github.io/rexpect)

The goal is to offer a similar set of functionality as [pexpect](https://pexpect.readthedocs.io/en/stable/overview.html).

# Basic usage

```
extern crate rexpect;

use rexpect::spawn;
use rexpect::errors::*;

fn ftp_job() -> Result<()> {
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

```

# Status

This project is still in early alpha, the api will still change a lot. Contributors more than welcome.

What already works:

- spawning a process through pty (threadsafe!)
- writing/reading to/from processes
- exit/kill processes

# Design decisions

- use [nix](https://github.com/nix-rust/nix) (and no libc directly) to keep the code safe and clean
- use error handling of []error-chain](https://github.com/brson/error-chain)