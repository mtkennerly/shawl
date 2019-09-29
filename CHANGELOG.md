## Unreleased

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
