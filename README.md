[![Build Status](https://api.travis-ci.org/philippkeller/rexpect.svg?branch=master)](https://travis-ci.org/philippkeller/rexpect)

[Documentation (Development)](http://philippkeller.github.io/rexpect)

The goal is to offer a similar set of functionality as [pexpect](https://pexpect.readthedocs.io/en/stable/overview.html).

# Basic usage

Add this to your `Cargo.toml` (sorry, not posted to crates.io yet)

```toml
[dependencies]
rexpect = {git = "https://github.com/philippkeller/rexpect"}
```

Simple example for interacting via ftp:

```rust
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
```

# Example with bash

```rust
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
```

# Project Status

What already works:

- spawning a processes through pty (threadsafe!), auto cleanup (killing all child processes)
- expect regex/string/EOF including timeouts
- spawning bash, interacting with ctrl-z, bg etc

What does not yet work:

- other repls as python are not implemented yet
- getting specific output (e.g. matching with regex and fetching the match) is missing

What will probably never be implemented

- screen/ANSI support ([deprecated](https://github.com/pexpect/pexpect/blob/master/pexpect/screen.py#L32) in pexpect anyway)

# Design decisions

- use error handling of [error-chain](https://github.com/brson/error-chain)
- use [nix](https://github.com/nix-rust/nix) (and avoid libc wherever possible) to keep the code safe and clean
- sadly, `expect` is used in rust too prominently to unwrap `Option`s and `Result`s, use `exp_*` instead