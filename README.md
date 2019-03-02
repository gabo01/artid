# Artid

[![Build Status][t1]][t2] [![Lines of code][l1]][l2]

[t1]: https://travis-ci.org/gabo01/artid.svg?branch=master
[t2]: https://travis-ci.org/gabo01/artid
[l1]: https://tokei.rs/b1/github/gabo01/artid
[l2]: https://github.com/gabo01/artid

A library for building light clients to backup your files. It's purpose is to make easier to make
and organize backups by specifying a configuration file and letting the program take care of the
rest.

## Getting started

This is the library of the project. For specific implementations see the contents of the
UI directory.

### Prerequisites

In order to compile the library, excluding potential dependencies needed for the frontends, you
will need:

- Rust version 1.31.0 or later
- Cargo verion 1.31.0 or later

### Building from Source

```bash
cargo build
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

## Contributing

Please see [CONTRIBUTING](.github/CONTRIBUTING.md) for a reference about the style guide, conventions
on code, tests and commit messages.

Any contributions you make will be automatically licensed under the MIT License unless told
otherwise.

The guidelines for contribution apply also to the implementations kept in the UI directory.

## Code of Conduct

Contribution to the project is organized under the terms of the Contributor Covenant, the
maintainers promise to intervene to uphold that code of conduct.

A copy of the code of conduct can be found locally [here][c1] or [online][c2].

[c1]: .github/CODE_OF_CONDUCT.md
[c2]: https://www.contributor-covenant.org/version/1/4/code-of-conduct.html

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for
details. Implementations of the library in this repo are also licensed under the MIT license
unless told otherwise.
