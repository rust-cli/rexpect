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

# Project Status

What already works:

- spawning a processes through pty (threadsafe!), auto cleanup (drop)
- expect regex/string/EOF including timeouts

What does not yet work:

- repl as in bash/ssh needs support
- sending ctrl-c, tab etc. (is just not implemented yet but easy to achieve)

What will probably never be implemented

- screen/ANSI support ([deprecated](https://github.com/pexpect/pexpect/blob/master/pexpect/screen.py#L32) in pexpect anyway)

# Design decisions

- use error handling of [error-chain](https://github.com/brson/error-chain)
- use [nix](https://github.com/nix-rust/nix) (and avoid libc wherever possible) to keep the code safe and clean
- sadly, `expect` is used in rust too prominently to unwrap `Option`s and `Result`s, use `exp_*` instead