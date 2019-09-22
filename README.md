# Shawl

Shawl is a service wrapper for Windows programs, written in Rust. You can set
it as the entry point of the service and simply pass the program for it to run
via CLI, without your program needing to be service-aware. Shawl's own options
and your command are separated by `--`:

```
shawl -- my-app.exe --foo bar
shawl --restart-ok --no-restart-err -- my-app.exe
```

Shawl will inspect the state of your program in order to report the correct
status to Windows. If your program exits with 0, the service will be considered
stopped, and any other code will be interpreted as a failure. By default,
Shawl will restart your program if it fails, but not if it exits with 0.

It differs from existing solutions like [WinSW](https://github.com/kohsuke/winsw)
and [NSSM](https://nssm.cc) in that their interfaces rely on running a special
install command to prepare the service, which means, for example, that you have
to run a `CustomAction` if you're installing with an MSI. With Shawl, you can
configure the service however you want, such as with the normal `ServiceInstall`
in an MSI, because Shawl doesn't have any special setup of its own.

## Development

Commands assume you are using [Git Bash](https://git-scm.com) on Windows:

* Add targets:
  * 32-bit: `rustup target add i686-pc-windows-msvc`
  * 64-bit: `rustup target add x86_64-pc-windows-msvc`
* Build:
  * 32-bit: `cargo build --release --target i686-pc-windows-msvc`
  * 64-bit: `cargo build --release --target x86_64-pc-windows-msvc`
* Test as a service:
  * Create: `sc create shawl-svc binPath= "$(readlink -f ./target/debug/shawl.exe) -- /path/to/app.exe"`
  * Inspect: `sc qc shawl-svc`
  * Start: `sc start shawl-svc`
  * Stop: `sc stop shawl-svc`
  * Delete: `sc delete shawl-svc`
