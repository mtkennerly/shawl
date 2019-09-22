# Shawl

Shawl is a service wrapper for Windows programs, written in Rust. You can
bundle Shawl with your project, set it as the entry point of the service, and
simply pass the command for it to run via CLI, without your program needing to
be service-aware. Shawl's own options and your command are separated by `--`:

```
shawl -- my-app.exe --foo bar
shawl --restart-ok --no-restart-err --stop-timeout 5000 -- my-app.exe
```

Shawl will inspect the state of your program in order to report the correct
status to Windows:

* If your program exits with 0, the service will be marked as stopped.
  You can pass `--restart-ok` to restart the command on code 0.
* If your program has a nonzero exit, Shawl will restart it, unless you pass
  `--no-restart-err`. If you tell Shawl to not restart it, or if the nonzero
  exit happens during a requested service stop, then that exit code will be
  reported to Windows as a service-specific error.
* When the service is requested to stop, Shawl sends your program a ctrl-C
  event, then waits up to 3 seconds (dependent on `--stop-timeout`) before
  forcibly killing the process if necessary.

It differs from existing solutions like [WinSW](https://github.com/kohsuke/winsw)
and [NSSM](https://nssm.cc) in that their interfaces rely on running a special
install command to prepare the service, which means, for example, that you have
to run a `CustomAction` if you're installing with an MSI. With Shawl, you can
configure the service however you want, such as with the normal `ServiceInstall`
in an MSI, because Shawl doesn't have any special setup of its own.

Prebuilt binaries are available on the
[releases page](https://github.com/mtkennerly/shawl/releases).

## CLI

```console
$ shawl --help
USAGE:
    shawl.exe [FLAGS] [OPTIONS] [--] <command>...

FLAGS:
    -h, --help              Prints help information
        --no-restart-err    Do not restart the wrapped program if it exits with a nonzero code
        --restart-ok        Restart the wrapped program if it exits with 0
    -V, --version           Prints version information

OPTIONS:
        --stop-timeout <stop-timeout>    How long to wait in milliseconds between sending the wrapped process a ctrl-C
                                         event and forcibly killing it [default: 3000]

ARGS:
    <command>...    Command to run as a service
```

## Development

Commands assume you are using [Git Bash](https://git-scm.com) on Windows:

* Add targets:
  * 32-bit: `rustup target add i686-pc-windows-msvc`
  * 64-bit: `rustup target add x86_64-pc-windows-msvc`
* Build:
  * 32-bit: `cargo build --release --target i686-pc-windows-msvc`
  * 64-bit: `cargo build --release --target x86_64-pc-windows-msvc`
* Test as a service:
  * Create: `sc create shawl binPath= "$(readlink -f ./target/debug/shawl.exe) -- $(readlink -f ./target/debug/child.exe | cut -c 3-)"`
    * Pass `--infinite` to the child to force a timeout on stop.
    * Pass `--exit 123` to the child to exit with that code.
  * Inspect: `sc qc shawl`
  * Start: `sc start shawl`
  * Stop: `sc stop shawl`
  * Delete: `sc delete shawl`
