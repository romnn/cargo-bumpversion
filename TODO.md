#### short-term

- remove "owned" python format string...
- make async
- implement pretty verbose mode
- proper error types
- lint
- rename repo to bumpversion only
- setup goreleaser
- release the bumpversion crate

#### long-term

- test and improve nom error messages
- remove deprecated code
- include spans in all the configs
- reduce confusion: remove version config, version spec, component spec, component, etc.
- more imperative, functional approach
- make serde-ini-spanend a separate crate

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
