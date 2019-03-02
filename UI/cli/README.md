# Artid CLI

This is the command line implementation of the artid library. The idea is to offer a small
but powerful command line tool that can be directly used or automated in order to backup data.

## Getting started

For the moment the project must be manually compiled in order to be used

### Prerequisites

- Rust version 1.31.0 or later
- Cargo version 1.31.0 or later

### Building from Source

In order to build the project run:

```bash
$ cargo build
```

Or, for real world use run:

```bash
$ cargo build --release
```

The binary will be inside the target folder so feel free to move it somewhere your $PATH variable
points to. In the future precompiled releases will be made available.

## Features

See the [README](../../README.md) at the top of the repository for the features available. The
implementation here will be kept in sync with the new features added to the library.

## Usage

```bash
artid 0.2.1
Gabriel Dos Ramos <dosramosgabriel@gmail.com>
Light client to backup your data

USAGE:
    artid [FLAGS] [SUBCOMMAND]

FLAGS:
    -b, --backtrace    Prints the complete error backtrace if an error is found
    -h, --help         Prints help information
    -V, --version      Prints version information

SUBCOMMANDS:
    backup     Updates the current backup or makes a new one if it does not exist
    help       Prints this message or the help of the given subcommand(s)
    restore    Restores all files of the backup to their original locations
```
