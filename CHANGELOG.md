## v1.1.0 (2022-01-18)

* Added version to executable properties.
* Added `--log-dir`.
  (Contributed by [oscarbailey-tc](https://github.com/mtkennerly/shawl/pull/19))
* Added `--env`.
* Added `--path`.
* When a custom `--cwd` is set, it is now automatically added to the command's
  PATH to make it easier to write some commands. Specifically, assuming there
  is a `C:\foo\bar\baz.exe`, then `--cwd C:\foo\bar -- baz.exe` will work now,
  but `--cwd C:\foo -- bar\baz.exe` still will not work, because the PATH only
  helps to resolve executable names, not subfolder names.

## v1.0.0 (2021-05-20)

* Shawl now handles computer shutdown/restart, allowing the wrapped program
  to exit gracefully.

## v0.6.2 (2021-03-09)

* Fixed an issue introduced in v0.6.1 where the 32-bit executable was not
  usable on 32-bit systems.
* Changed build process to avoid potential "VCRUNTIME140_1.dll was not found"
  error when using the program.

## v0.6.1 (2020-12-22)

* Updated `windows-service` dependency to avoid a build failure where
  `err-derive` would use a private symbol from `quote`.

## v0.6.0 (2020-03-22)

* Added `--pass-start-args`.
  (Contributed by [Enet4](https://github.com/mtkennerly/shawl/pull/6))
* Added log rotation and service-specific log files.

## v0.5.0 (2020-03-03)

* Added logging of stdout and stderr from commands.
* Added `--no-log` and `--no-log-cmd` options to configure logging.

## v0.4.0 (2019-10-05)

* Added `--cwd` for setting the command's working directory.
* Set default help text width to 80 characters.
* Fixed issue where Shawl would not report an error if it was unable to
  launch the command (e.g., file not found).
* Fixed missing quotes when adding a service if the name or any part of
  the command contained inner spaces.
* Fixed `--pass` and `--stop-timeout` being added to the service command
  configured by `shawl add` even when not explicitly set.

## v0.3.0 (2019-09-30)

* Added `shawl add` for quickly creating a Shawl-wrapped service.
* Moved existing CLI functionality under `shawl run`.
* Generalized `--restart-ok` and `--no-restart-err` into
  `--(no-)restart` and `--restart-if(-not)`.
* Added `--pass` to customize which exit codes are considered successful.

## v0.2.0 (2019-09-22)

* Send ctrl-C to child process first instead of always forcibly killing it.
* Report command failure as a service-specific error to Windows.
* Added `--stop-timeout` option.

## v0.1.0 (2019-09-22)

* Initial release.
