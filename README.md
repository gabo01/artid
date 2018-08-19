# Rust Backup [![Build Status](https://travis-ci.org/gabo01/rust-backup.svg?branch=master)](https://travis-ci.org/gabo01/rust-backup)

A light client to backup your files. It's purpouse is to make easier to make and organize backups
by specifying a configuration file and letting the program take care of the rest.

## Getting started

### Prerequisites

Rust version 1.27.1 or later

Cargo verion 1.27.0 or later

### Building from Source

```
cargo build
```
or just
```
cargo run -- [ARGS]
```

### Debugging

The debug folder + the .vscode folder contain a basic configuration for debugging the
the program using vscode built-in debugger. In order to set the debugger for rust code
use the following links:

[Setting a rust developing env](https://hoverbear.org/2017/03/03/setting-up-a-rust-devenv/)

During the debug process the debug folder contents will change, this changes are not to be
submitted to the repo. To avoid tracking this changes on your local copy run the following
command on your local repo:

```
git update-index --assume-unchanged debug/**/
```

## Features

- [x] Command line client
- [ ] Versioned backups
- [ ] Zip the backups made
- [ ] GUI client
- [ ] Integration with Mega, Dropbox and Google Drive
- [ ] Encryption of sensible files

## Usage from CLI

```
backup-rs 0.1.0
Gabriel Dos Ramos <dosramosgabriel@gmail.com>
Light client to backup your data

USAGE:
    backup-rs [FLAGS] [SUBCOMMAND]

FLAGS:
    -b, --backtrace    Prints the complete error backtrace if an error is found
    -h, --help         Prints help information
    -V, --version      Prints version information

SUBCOMMANDS:
    help       Prints this message or the help of the given subcommand(s)
    restore    Restores all files of the backup to their original locations
    update     Updates the current backup or makes a new one if it does not exist
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details
