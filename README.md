# Artid

[![Build Status][t1]][t2] [![Lines of code][l1]][l2]

[t1]: https://travis-ci.org/gabo01/artid.svg?branch=master
[t2]: https://travis-ci.org/gabo01/artid
[l1]: https://tokei.rs/b1/github/gabo01/artid
[l2]: https://github.com/gabo01/artid

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
artid 0.1.0
Gabriel Dos Ramos <dosramosgabriel@gmail.com>
Light client to backup your data

USAGE:
    artid [FLAGS] [SUBCOMMAND]

FLAGS:
    -b, --backtrace    Prints the complete error backtrace if an error is found
    -h, --help         Prints help information
    -V, --version      Prints version information

SUBCOMMANDS:
    help       Prints this message or the help of the given subcommand(s)
    restore    Restores all files of the backup to their original locations
    update     Updates the current backup or makes a new one if it does not exist
```

## Contributing

Please see [CONTRIBUTING](.github/CONTRIBUTING.md) for a reference about the style guide, conventions
on code, tests and commit messages.

Any contributions you make will be automatically licensed under the MIT License unless told
otherwise.

## Code of Conduct

Contribution to the project is organized under the terms of the Contributor Covenant, the
maintainers promise to intervene to uphold that code of conduct.

A copy of the code of conduct can be found locally [here][c1] or [online][c2].

[c1]: .github/CODE_OF_CONDUCT.md
[c2]: https://www.contributor-covenant.org/version/1/4/code-of-conduct.html

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details
