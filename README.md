# A light client to make and organize file backups

A tool to organize your backup files. It's purpouse is to make easier to make a backup by specifying beforehand the
list of directories you want to backup in a config file

## Getting started

### Prerequisites

Rust version 1.27.1 or newer

Cargo verion 1.27.0 or newer

### Building from Source

```
cargo build
```
or just
```
cargo run -- [ARGS]
```

## Usage

```
backup 0.1.0
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
