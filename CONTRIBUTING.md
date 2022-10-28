## Development
Rust 1.38.0 or newer is recommended.

* Run tests (avoid concurrency since the integration tests make real services):
  * `cargo test -- --test-threads 1`
* Activate pre-commit hooks (requires Python) to handle formatting/linting:
  ```
  pip install --user pre-commit
  pre-commit install
  ```

## Release
Commands assume you are using [Git Bash](https://git-scm.com) on Windows:

* Add targets:
  * 32-bit: `rustup target add i686-pc-windows-msvc`
  * 64-bit: `rustup target add x86_64-pc-windows-msvc`
* Install tool for generating license bundle:
  * `cargo install cargo-lichking`
* Prepare release:
  ```
  export VERSION=$(cargo pkgid | cut -d# -f2 | cut -d: -f2)
  rm -rf dist
  mkdir dist
  cargo build --release --target i686-pc-windows-msvc
  cargo build --release --target x86_64-pc-windows-msvc
  cp target/i686-pc-windows-msvc/release/shawl.exe dist/shawl-v$VERSION-win32.exe
  cp target/x86_64-pc-windows-msvc/release/shawl.exe dist/shawl-v$VERSION-win64.exe
  cargo lichking bundle --file dist/shawl-v$VERSION-legal.txt
  sed -i -E 's/\\\\\?\\C:\\Users\\[^\\]+/~/g' dist/shawl-v$VERSION-legal.txt
  ```
