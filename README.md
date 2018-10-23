# Rust Backup

[![Build Status][t1]][t2] [![Lines of code][l1]][l2]

[t1]: https://travis-ci.org/gabo01/rust-backup.svg?branch=master
[t2]: https://travis-ci.org/gabo01/rust-backup
[l1]: https://tokei.rs/b1/github/gabo01/rust-backup
[l2]: https://github.com/gabo01/rust-backup

A light client to backup your files. It's purpose is to make easier to make and organize backups
by specifying a configuration file and letting the program take care of the rest.

## Getting started

Right now, the only way to use the project is to compile it yourself. Once it reaches maturity,
precompiled binaries will be made available in the releases section.

### Prerequisites

- Rust version 1.27.1 or later
- Cargo verion 1.27.0 or later

### Building from Source

```bash
cargo build
```

or just

```bash
cargo run -- [ARGS]
```

### Debugging

The debug folder + the .vscode folder contain a basic configuration for debugging the
the program using vscode built-in debugger. In order to set the debugger for rust code
use the following links:

[Setting a rust developing env](https://hoverbear.org/2017/03/03/setting-up-a-rust-devenv/)

During the debug process the debug folder contents will change, these changes are not to be
submitted to the repo. To avoid tracking these changes on your local copy run the following
command on your local repo:

```bash
git update-index --assume-unchanged debug/**/
```

## Features

- [x] Command line client
- [x] Versioned backups
- [ ] Zip the backups made
- [ ] GUI client
- [ ] Integration with Mega, Dropbox and Google Drive
- [ ] Encryption of sensible files

**Disclaimer:** The features checked are not stable yet and won't be as long as the application
version does not reach 1.0.0

## Usage from CLI

```bash
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
