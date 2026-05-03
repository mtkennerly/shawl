## Development
Use the latest version of Rust.

* Run tests:
  * `cargo test`
* Activate pre-commit hooks (requires Python) to handle formatting/linting:
  ```
  pip install --user pre-commit
  pre-commit install
  ```
* Generate docs (requires Python):
  ```
  pip install --user invoke tomli
  invoke docs
  ```

## Release
Commands assume you are using [Git Bash](https://git-scm.com) on Windows:

### Dependencies (one-time)
```bash
pip install invoke
cargo install cargo-lichking
```

### Process
* Run `invoke prerelease`
* Run `git add` for all relevant changes
* Run `invoke release`
  * This will create a new commit/tag and push them.
  * Manually create a release on GitHub and attach the workflow build artifacts
    (plus `dist/*-legal.zip`).
* Run `cargo publish`
* Run `invoke release-winget`
  * This will automatically push a branch to a fork of https://github.com/microsoft/winget-pkgs .
  * Manually open a pull request for that branch.
