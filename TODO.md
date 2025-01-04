#### short-term

- split up into cargo-bumpversion bumpversion-cli bumpversion crates
- implement pretty verbose mode
- proper error types
- lint
- rename repo to bumpversion only
- setup goreleaser
- release the bumpversion crate

#### long-term

- make async
- test and improve nom error messages
- remove deprecated code
- include spans in all the configs
- reduce confusion: remove version config, version spec, component spec, component, etc.
- more imperative, functional approach
- make serde-ini-spanend a separate crate

- DONE: implement basic version bumping
- DONE: use toml_spanned for better diagnostics?
- DONE: test with python how ini files should handle booleans (is "True" valid?)
- DONE: use spanned ini parser?
