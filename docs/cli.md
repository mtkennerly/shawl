This is the raw help text for the command line interface.

## `--help`
```
shawl
Wrap arbitrary commands as Windows services

USAGE:
    shawl.exe
    shawl.exe <SUBCOMMAND>

OPTIONS:
    -h, --help    Print help information

SUBCOMMANDS:
    add     Add a new service
    help    Print this message or the help of the given subcommand(s)
    run     Run a command as a service; only works when launched by the
                Windows service manager
```

## `add --help`
```
shawl.exe-add
Add a new service

USAGE:
    shawl.exe add [OPTIONS] --name <NAME> [--] <COMMAND>...

ARGS:
    <COMMAND>...    Command to run as a service

OPTIONS:
        --cwd <path>
            Working directory in which to run the command. You may provide a
            relative path, and it will be converted to an absolute one

        --dependencies <DEPENDENCIES>
            Other services that must be started first (comma-separated)

        --env <ENV>
            Additional environment variable in the format 'KEY=value'
            (repeatable)

    -h, --help
            Print help information

        --log-dir <path>
            Write log file to a custom directory. This directory will be created
            if it doesn't exist

        --name <NAME>
            Name of the service to create

        --no-log
            Disable all of Shawl's logging

        --no-log-cmd
            Disable logging of output from the command running as a service

        --no-restart
            Never restart the command regardless of the exit code

        --pass <codes>
            Exit codes that should be considered successful (comma-separated)
            [default: 0]

        --pass-start-args
            Append the service start arguments to the command

        --path <PATH>
            Additional directory to add to the PATH environment variable
            (repeatable)

        --priority <PRIORITY>
            Process priority of the command to run as a service [possible
            values: realtime, high, above-normal, normal, below-normal, idle]

        --restart
            Always restart the command regardless of the exit code

        --restart-if <codes>
            Restart the command if the exit code is one of these
            (comma-separated)

        --restart-if-not <codes>
            Restart the command if the exit code is not one of these
            (comma-separated)

        --stop-timeout <ms>
            How long to wait in milliseconds between sending the wrapped process
            a ctrl-C event and forcibly killing it [default: 3000]
```

## `run --help`
```
shawl.exe-run
Run a command as a service; only works when launched by the Windows service
manager

USAGE:
    shawl.exe run [OPTIONS] [--] <COMMAND>...

ARGS:
    <COMMAND>...    Command to run as a service

OPTIONS:
        --cwd <path>
            Working directory in which to run the command. Must be an absolute
            path

        --env <ENV>
            Additional environment variable in the format 'KEY=value'
            (repeatable)

    -h, --help
            Print help information

        --log-dir <path>
            Write log file to a custom directory. This directory will be created
            if it doesn't exist

        --name <NAME>
            Name of the service; used in logging, but does not need to match
            real name [default: Shawl]

        --no-log
            Disable all of Shawl's logging

        --no-log-cmd
            Disable logging of output from the command running as a service

        --no-restart
            Never restart the command regardless of the exit code

        --pass <codes>
            Exit codes that should be considered successful (comma-separated)
            [default: 0]

        --pass-start-args
            Append the service start arguments to the command

        --path <PATH>
            Additional directory to add to the PATH environment variable
            (repeatable)

        --priority <PRIORITY>
            Process priority of the command to run as a service [possible
            values: realtime, high, above-normal, normal, below-normal, idle]

        --restart
            Always restart the command regardless of the exit code

        --restart-if <codes>
            Restart the command if the exit code is one of these
            (comma-separated)

        --restart-if-not <codes>
            Restart the command if the exit code is not one of these
            (comma-separated)

        --stop-timeout <ms>
            How long to wait in milliseconds between sending the wrapped process
            a ctrl-C event and forcibly killing it [default: 3000]
```
