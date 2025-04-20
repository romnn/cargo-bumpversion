## bumpversion

[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/romnn/bumpversion/build.yaml?branch=main&label=build">](https://github.com/romnn/bumpversion/actions/workflows/build.yaml)
[<img alt="test status" src="https://img.shields.io/github/actions/workflow/status/romnn/bumpversion/test.yaml?branch=main&label=test">](https://github.com/romnn/bumpversion/actions/workflows/test.yaml)
[![dependency status](https://deps.rs/repo/github/romnn/bumpversion/status.svg)](https://deps.rs/repo/github/romnn/bumpversion)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/bumpversion/latest?label=docs.rs">](https://docs.rs/bumpversion)
[<img alt="crates.io" src="https://img.shields.io/crates/v/bumpversion">](https://crates.io/crates/bumpversion)

This is an improved version of the popular [callowayproject/bump-my-version](https://github.com/callowayproject/bump-my-version) (formerly [peritus/bumpversion](https://github.com/peritus/bumpversion) and [c4urself/bump2version](https://github.com/c4urself/bump2version)) written in Rust.

#### Features

- No more global `pip` installs! Easy to install via `brew`, `cargo`, or precompiled static binary.
- Fully compatible with your existing configuration:
    - `pyproject.toml`
    - `.bumpversion.toml`
    - `.bumpversion.cfg`
    - `setup.cfg`
    - `Cargo.toml` (planned)

### Installation

```bash
# will install `bumpversion` binary
brew install romnn/tap/bumpversion

# will install `cargo-bumpversion` binary
brew install romnn/tap/cargo-bumpversion

# or install from source (will install both `cargo-bumpversion` and `bumpversion` binaries)
cargo install bumpversion-cli
```

### CLI usage

You can use this as a drop-in replacement for the Python `bumpversion`, `bump2version`, or `bump-my-version`.
For usage instructions, please refer to [the Python version](https://github.com/callowayproject/bump-my-version).

#### Development

```bash
cargo run -- --dir ../dir/to/a/repo/with/.bumpversion.toml --verbose --dry-run patch
```
