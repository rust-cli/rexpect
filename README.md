[![Build Status](https://api.travis-ci.org/philippkeller/rexpect.svg?branch=master)](https://travis-ci.org/philippkeller/rexpect)

[Documentation (Development)](http://philippkeller.github.io/rexpect)

The goal is to offer a similar set of functionality as [pexpect](https://pexpect.readthedocs.io/en/stable/overview.html).

# Status

This project is still in early alpha, the api will still change a lot. Contributors more than welcome.

What already works:

- spawning a process through pty (threadsafe!)
- writing/reading to/from processes
- exit/kill processes

# Design decisions

- use [nix](https://github.com/nix-rust/nix) (and no libc directly) to keep the code safe and clean
- use error handling of []error-chain](https://github.com/brson/error-chain)