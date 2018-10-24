# How to Contribute

Hi, contributions are always welcome! Before opening a pull request, [create an issue][i1] so
potential changes can be discussed before making a pull request with changes that may not be
accepted.

You can also help solving existing issues. In order to do that go to the issue and tell you want
to take care of it to avoid multiple people working on the same thing.

[i1]: https://github.com/gabo01/rust-backup/issues

## Bug Reports

For bug reports, try to provide a minimal working example of the bug and describe the buggy
behaviour versus the expected behaviour. Remember that a clearly reproducible bug is both easier
and more probable to be fixed than a hard to reproduce bug.

In case of security bugs, please send me an email instead of opening an issue to avoid initial
public disclosure.

## Adding new features

If you wish to add a new feature, provide an use case for the addition and why it can't be done / is
hard to do using the existing application.

Consider also that you will be asked about how the new feature should work, look and feel before
being finally implemented.

## Developing

Below is a list of the different elements that have to be taken in account while adding to the
existing code base:

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

### Testing

When designing tests, we mostly adopt these [rules][rules]. However, rust's ecosystem has some
particular design elements that make full adoption impossible in some cases; so we have a general
set of rules to follow:

- The library's public API is tested through *integration tests*
- If testing only a part of the public's method functionality, use *unit tests*
- For private methods, use *unit tests*
- On *unit tests*, try to avoid: filesystem, network or database calls
- If an external call is made in a unit test and it slows the test down, add the `#[ignore]` flag
- If a unit test involves expensive operations or *thread sleeping*, add the `#[ignore]` flag

Aside from that, is also important to notice that we group tests according to the functionality they
test **even if the functionality is grouped in the same module**. This means that tests in every
module are splitted in groups depending on what they test.

[rules]: https://www.artima.com/weblogs/viewpost.jsp?thread=126923

### Styling

Please follow the rust's community standard on your code. We use *both* **clippy** and **rustfmt**
in our CI builds. This means than non formatted code will simply not pass the build and won't be
merged until it is well formatted.

The verions of clippy and rustfmt that the CI uses are those corresponding to the latest stable
release of rust. This means that for developing you will need the latest version of the stable
compiler to check formatting and lints.
