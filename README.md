## bumpversion

[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/romnn/cargo-bumpversion/build.yml?branch=main&label=build">](https://github.com/romnn/cargo-bumpversion/actions/workflows/build.yml)
[<img alt="test status" src="https://img.shields.io/github/actions/workflow/status/romnn/cargo-bumpversion/test.yml?branch=main&label=test">](https://github.com/romnn/cargo-bumpversion/actions/workflows/test.yml)
[<img alt="crates.io" src="https://img.shields.io/crates/v/bumpversion">](https://crates.io/crates/bumpversion)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/bumpversion/latest?label=docs.rs">](https://docs.rs/taski)

This is an improved version of the popular Python package `bumpversion` (now maintained under [`bump2version`](https://github.com/c4urself/bump2version)) written in Rust.

It is fully compatible with your existing configuration in `.bumpversion.cfg` or `setup.cfg` and includes both a Rust library and a command line utility, with usage instructions below.

### Improvements

The main added benefit of this library is the ability to use it as a library component for your local build and deployment scripts.

Often, people tag a new release using `bumpversion` and push a tag into CI (e.g. GitHub actions).
But what if your project requires a lot of data that is not accessible from the CI/CD host?

You could use this library to write build scripts using the pre and post hooks provided to e.g. build and package your application and upon success tag a new release to be pushed into CI for deploying the packages built.

### CLI usage

You can also just use this version as a drop-in replacement for the Python `bump2version`.

Install it with

```bash
cargo install bumpversion
```

For usage instructions, please refer to [the Python version](https://github.com/c4urself/bump2version).

#### Library usage

TODO

#### TODO

- add a git2 backend and make the tests into macros
- add the versioning functions
- add the config file parsing
- add the main CLI loop
