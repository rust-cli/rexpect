# Change Log

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).

## Unreleased

- if process doesn't end on SIGTERM a `kill -9` is sent after timeout is elapsed

## [0.2.0] 2017-09-20

### Changed

All `exp_*` methods now also return the yet unread string and/or the matched string:

- `exp_string`: return the yet unread string
- `exp_regex`: return a tuple of (yet unread string, matched string)
- `exp_eof` and `exp_nbytes`: return the yet unread string

### Fixed

- each execution of rexpect left a temporary file in /tmp/ this is now no longer the case
- try_read was blocking when there was no char ready (!) -> fixed