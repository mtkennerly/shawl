This is the raw help text for the command line interface.

## `--help`
```
Wrap arbitrary commands as Windows services

Usage: shawl.exe
       shawl.exe <COMMAND>

Commands:
  add
          Add a new service
  run
          Run a command as a service; only works when launched by the Windows service manager
  help
          Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help
  -V, --version
          Print version
```

## `add --help`
```
Add a new service

Usage: shawl.exe add [OPTIONS] --name <NAME> -- <COMMAND>...

Arguments:
  <COMMAND>...
          Command to run as a service

Options:
      --pass <codes>
          Exit codes that should be considered successful (comma-separated) [default: 0]
      --restart
          Always restart the command regardless of the exit code
      --no-restart
          Never restart the command regardless of the exit code
      --restart-if <codes>
          Restart the command if the exit code is one of these (comma-separated)
      --restart-if-not <codes>
          Restart the command if the exit code is not one of these (comma-separated)
      --restart-delay <ms>
          How long to wait before restarting the wrapped process
      --stop-timeout <ms>
          How long to wait in milliseconds between sending the wrapped process a ctrl-C event and
          forcibly killing it [default: 3000]
      --no-log
          Disable all of Shawl's logging
      --no-log-cmd
          Disable logging of output from the command running as a service
      --log-dir <path>
          Write log file to a custom directory. This directory will be created if it doesn't exist
      --log-as <LOG_AS>
          Use a different name for the main log file. Set this to just the desired base name of the
          log file. For example, `--log-as shawl` would result in a log file named
          `shawl_rCURRENT.log` instead of the normal `shawl_for_<name>_rCURRENT.log` pattern
      --log-cmd-as <LOG_CMD_AS>
          Use a separate log file for the wrapped command's stdout and stderr. Set this to just the
          desired base name of the log file. For example, `--log-cmd-as foo` would result in a log
          file named `foo_rCURRENT.log`. The output will be logged as-is without any additional log
          template
      --log-rotate <LOG_ROTATE>
          Threshold for rotating log files. Valid options: `daily`, `hourly`, `bytes=n` (every N
          bytes) [default: bytes=2097152]
      --log-retain <LOG_RETAIN>
          How many old log files to retain [default: 2]
      --pass-start-args
          Append the service start arguments to the command
      --env <ENV>
          Additional environment variable in the format 'KEY=value' (repeatable)
      --path <PATH>
          Additional directory to append to the PATH environment variable (repeatable)
      --path-prepend <path>
          Additional directory to prepend to the PATH environment variable (repeatable)
      --priority <PRIORITY>
          Process priority of the command to run as a service [possible values: realtime, high,
          above-normal, normal, below-normal, idle]
      --kill-process-tree
          Kill the entire process tree when the service stops. Uses a Windows Job Object to track
          and terminate all child processes automatically
      --cwd <path>
          Working directory in which to run the command. You may provide a relative path, and it
          will be converted to an absolute one
      --dependencies <DEPENDENCIES>
          Other services that must be started first (comma-separated)
      --name <NAME>
          Name of the service to create
  -h, --help
          Print help
```

## `run --help`
```
Run a command as a service; only works when launched by the Windows service manager

Usage: shawl.exe run [OPTIONS] -- <COMMAND>...

Arguments:
  <COMMAND>...
          Command to run as a service

Options:
      --pass <codes>
          Exit codes that should be considered successful (comma-separated) [default: 0]
      --restart
          Always restart the command regardless of the exit code
      --no-restart
          Never restart the command regardless of the exit code
      --restart-if <codes>
          Restart the command if the exit code is one of these (comma-separated)
      --restart-if-not <codes>
          Restart the command if the exit code is not one of these (comma-separated)
      --restart-delay <ms>
          How long to wait before restarting the wrapped process
      --stop-timeout <ms>
          How long to wait in milliseconds between sending the wrapped process a ctrl-C event and
          forcibly killing it [default: 3000]
      --no-log
          Disable all of Shawl's logging
      --no-log-cmd
          Disable logging of output from the command running as a service
      --log-dir <path>
          Write log file to a custom directory. This directory will be created if it doesn't exist
      --log-as <LOG_AS>
          Use a different name for the main log file. Set this to just the desired base name of the
          log file. For example, `--log-as shawl` would result in a log file named
          `shawl_rCURRENT.log` instead of the normal `shawl_for_<name>_rCURRENT.log` pattern
      --log-cmd-as <LOG_CMD_AS>
          Use a separate log file for the wrapped command's stdout and stderr. Set this to just the
          desired base name of the log file. For example, `--log-cmd-as foo` would result in a log
          file named `foo_rCURRENT.log`. The output will be logged as-is without any additional log
          template
      --log-rotate <LOG_ROTATE>
          Threshold for rotating log files. Valid options: `daily`, `hourly`, `bytes=n` (every N
          bytes) [default: bytes=2097152]
      --log-retain <LOG_RETAIN>
          How many old log files to retain [default: 2]
      --pass-start-args
          Append the service start arguments to the command
      --env <ENV>
          Additional environment variable in the format 'KEY=value' (repeatable)
      --path <PATH>
          Additional directory to append to the PATH environment variable (repeatable)
      --path-prepend <path>
          Additional directory to prepend to the PATH environment variable (repeatable)
      --priority <PRIORITY>
          Process priority of the command to run as a service [possible values: realtime, high,
          above-normal, normal, below-normal, idle]
      --kill-process-tree
          Kill the entire process tree when the service stops. Uses a Windows Job Object to track
          and terminate all child processes automatically
      --cwd <path>
          Working directory in which to run the command. Must be an absolute path
      --name <NAME>
          Name of the service; used in logging, but does not need to match real name [default:
          Shawl]
  -h, --help
          Print help
```
