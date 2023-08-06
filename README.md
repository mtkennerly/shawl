# Shawl

Shawl is a wrapper for running arbitrary programs as Windows services,
written in Rust. It handles the Windows service API for you so that your
program only needs to respond to ctrl-C/SIGINT. If you're creating a project
that needs to run as a service, simply bundle Shawl with your project, set it
as the entry point, and pass the command to run via CLI. Here is an example of
creating a service wrapped with Shawl (note that `--` separates Shawl's own
options from the command that you'd like it to run):

* Using Shawl's `add` command:
  * `shawl add --name my-app -- C:/path/my-app.exe`
* Using the Windows `sc` command for more control:
  * `sc create my-app binPath= "C:/path/shawl.exe run -- C:/path/my-app.exe"`
* Then start or configure the service as normal:
  * ```
    sc config my-app start= auto
    sc start my-app
    ```

Shawl will inspect the state of your program in order to report the correct
status to Windows:

* By default, when your program exits, Shawl will restart it if the exit code
  is nonzero. You can customize this behavior with `--(no-)restart` for all
  exit codes or `--restart-if(-not)` for specific exit codes.
  Note that these four options are mutually exclusive.
* When the service is requested to stop, Shawl sends your program a ctrl-C
  event, then waits up to 3000 milliseconds (based on `--stop-timeout`) before
  forcibly killing the process if necessary.
* In either case, if Shawl is not restarting your program, then it reports
  the exit code to Windows as a service-specific error, unless the exit code
  is 0 or a code you've configured with `--pass`.

Shawl creates a log file for each service, `shawl_for_<service>_*.log`, in the
same location as the Shawl executable, with both its own messages and the output
from the commands that it runs. If anything goes wrong, you can read the log to
find out more. You can disable all logging with `--no-log`, and you can disable
just the command logs with `--no-log-cmd`. Each log file is limited to 2 MB, and
up to 2 rotated copies will be retained.

Shawl differs from existing solutions like [WinSW](https://github.com/kohsuke/winsw)
and [NSSM](https://nssm.cc) in that they require running a special install
command to prepare the service, which means, for example, that you have to run
a `CustomAction` if you need to install a service in an MSI. With Shawl, you can
configure the service however you want, such as with the normal `ServiceInstall`
in an MSI or by running `sc create`, because Shawl doesn't have any special
setup of its own. The `shawl add` command is just an optional convenience.

Bear in mind that the default account for new services is the Local System
account, which has a different `PATH` environment variable than your user
account. If you configure Shawl to run a command like `npm start`, that means
`npm` needs to be in the Local System account's `PATH`, or you could also
change the account used by the service instead.

Also note that running a service with a Local System account is as **dangerous**
as running a Unix service as root. This greatly increases the risk of your system
being hacked if you expose a port to the public for the service you are going to 
wrap. It is recommended that you use a restricted account, such as [Network Service](
https://learn.microsoft.com/en-us/windows/win32/services/networkservice-account),
to run services. To do this, first grant the Network Service account read, write, 
and execute permissions on Shawl’s installation directory, and then execute 
`sc config my-app obj= "NT AUTHORITY\Network Service" password= ""`. If the service
needs to read and write files, you may also need to grant the Network Service 
permissions to the directory that the service wants to access. More information 
about Windows service user accounts can be found [here](
https://stackoverflow.com/questions/510170/the-difference-between-the-local-system-account-and-the-network-service-acco).

## Installation
* Prebuilt binaries are available on the
  [releases page](https://github.com/mtkennerly/shawl/releases).
  It's portable, so you can simply download it and put it anywhere
  without going through an installer.
* If you have Rust installed, you can run `cargo install shawl`.
* If you have [Scoop](https://scoop.sh), you can install by running:

  ```
  scoop bucket add extras
  scoop install shawl
  ```

  To update, run:

  ```
  scoop update
  scoop update shawl
  ```

## CLI
Below is the top-level help output (from `shawl --help`).
For help with specific subcommands, you can add the command name (e.g., `shawl add --help`).

```console
$ shawl --help
Wrap arbitrary commands as Windows services

USAGE:
    shawl.exe
    shawl.exe <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    add     Add a new service
    help    Prints this message or the help of the given subcommand(s)
    run     Run a command as a service; only works when launched by the
            Windows service manager
```

## Development
Please refer to [CONTRIBUTING.md](CONTRIBUTING.md).
