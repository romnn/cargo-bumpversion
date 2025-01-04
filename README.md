## bumpversion

[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/romnn/cargo-bumpversion/build.yaml?branch=main&label=build">](https://github.com/romnn/cargo-bumpversion/actions/workflows/build.yaml)
[<img alt="test status" src="https://img.shields.io/github/actions/workflow/status/romnn/cargo-bumpversion/test.yaml?branch=main&label=test">](https://github.com/romnn/cargo-bumpversion/actions/workflows/test.yaml)
[![dependency status](https://deps.rs/repo/github/romnn/cargo-bumpversion/status.svg)](https://deps.rs/repo/github/romnn/cargo-bumpversion)
[<img alt="crates.io" src="https://img.shields.io/crates/v/bumpversion">](https://crates.io/crates/bumpversion)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/bumpversion/latest?label=docs.rs">](https://docs.rs/taski)

This is an improved version of the popular [callowayproject/bump-my-version](https://github.com/callowayproject/bump-my-version) (formerly [peritus/bumpversion](https://github.com/peritus/bumpversion) and [c4urself/bump2version](https://github.com/c4urself/bump2version)) written in Rust.

#### Features

- No more global `pip` installs! Easy to install via `brew`, `cargo`, or precompiled static binary.
- Fully compatible with your existing configuration in `.bumpversion.cfg`, `setup.cfg`, or `pyproject.toml`
- Also supports configuration in your `Cargo.toml`
- Additional hook system

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

# TODO:
brew install ...
```

For usage instructions, please refer to [the Python version](https://github.com/callowayproject/bump-my-version).

#### Development

```bash
cargo run -- --dir ../cargo-feature-combinations/ --log-level trace patch
```

#### TODO

- add a git2 backend and make the tests into macros
- add the versioning functions
- add the config file parsing
- add the main CLI loop
