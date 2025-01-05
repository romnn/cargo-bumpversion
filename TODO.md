#### short-term

- implement pretty verbose mode
- lint
- read configuration from Cargo.toml
- rename repo to bumpversion only
- setup goreleaser
- release the bumpversion crate

#### long-term

- remove different log formats
- test and improve nom error messages
- include spans in all the configs
- publish serde-ini-spanend as a separate crate

- DONE: remove deprecated code
- DONE: proper error types
- DONE: reduce confusion: remove version config, version spec, component spec, component, etc.
- DONE: more imperative, functional approach
- DONE: make async
- DONE: remove "owned" python format string...
- DONE: group file changes together
- DONE: get rid of configured file
- DONE: move do_bump to lib
- DONE: make compat version the default
- DONE: remove bump type
- DONE: split up into cargo-bumpversion bumpversion-cli bumpversion crates
- DONE: implement basic version bumping
- DONE: use toml_spanned for better diagnostics?
- DONE: test with python how ini files should handle booleans (is "True" valid?)
- DONE: use spanned ini parser?
