## Unreleased

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
