#[cfg(all(test, windows))]
speculate::speculate! {
    fn parent() -> String {
        format!("{}/target/debug/shawl.exe", env!("CARGO_MANIFEST_DIR"))
    }

    fn child() -> String {
        format!("{}/target/debug/shawl-child.exe", env!("CARGO_MANIFEST_DIR"))
    }

    fn log_file() -> String {
        format!("{}/target/debug/shawl.log", env!("CARGO_MANIFEST_DIR"))
    }

    fn delete_log() {
        if log_exists() {
            std::fs::remove_file(log_file()).unwrap();
        }
    }

    fn log_exists() -> bool {
        std::path::Path::new(&log_file()).exists()
    }

    fn run_cmd(args: &[&str]) -> std::process::Output {
        let out = std::process::Command::new(args[0])
                .args(args[1..].iter())
                .output()
                .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        out
    }

    fn run_shawl(args: &[&str]) -> std::process::Output {
        let out = std::process::Command::new(parent())
                .args(args)
                .output()
                .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        out
    }

    before {
        run_cmd(&["sc", "stop", "shawl"]);
        run_cmd(&["sc", "delete", "shawl"]);
    }

    after {
        run_cmd(&["sc", "stop", "shawl"]);
        run_cmd(&["sc", "delete", "shawl"]);
    }

    describe "shawl add" {
        it "works with minimal arguments" {
            let shawl_output = run_shawl(&["add", "--name", "shawl", "--", &child()]);
            assert_eq!(shawl_output.status.code(), Some(0));

            let sc_output = run_cmd(&["sc", "qc", "shawl"]);
            let pattern = regex::Regex::new(
                r"BINARY_PATH_NAME *: .+shawl\.exe run --name shawl -- .+shawl-child\.exe"
            ).unwrap();
            assert!(pattern.is_match(&String::from_utf8_lossy(&sc_output.stdout)));
        }

        it "handles command parts with spaces" {
            let shawl_output = run_shawl(&["add", "--name", "shawl", "--", "foo bar", "--baz"]);
            assert_eq!(shawl_output.status.code(), Some(0));

            let sc_output = run_cmd(&["sc", "qc", "shawl"]);
            let pattern = regex::Regex::new(
                r#"BINARY_PATH_NAME *: .+shawl\.exe run --name shawl -- "foo bar" --baz"#
            ).unwrap();
            assert!(pattern.is_match(&String::from_utf8_lossy(&sc_output.stdout)));
        }

        it "rejects nonexistent --cwd path" {
            let shawl_output = run_shawl(&["add", "--name", "shawl", "--cwd", "shawl-fake", "--", &child()]);
            assert_eq!(shawl_output.status.code(), Some(1));
        }
    }

    describe "shawl run" {
        it "handles a successful command" {
            run_shawl(&["add", "--name", "shawl", "--", &child()]);
            run_cmd(&["sc", "start", "shawl"]);
            run_cmd(&["sc", "stop", "shawl"]);

            let sc_output = run_cmd(&["sc", "query", "shawl"]);
            let stdout = String::from_utf8_lossy(&sc_output.stdout);

            assert!(stdout.contains("STATE              : 1  STOPPED"));
            assert!(stdout.contains("WIN32_EXIT_CODE    : 0  (0x0)"));
        }

        it "reports a --pass code as success" {
            run_shawl(&["add", "--name", "shawl", "--pass", "1", "--", &child(), "--exit", "1"]);
            run_cmd(&["sc", "start", "shawl"]);
            run_cmd(&["sc", "stop", "shawl"]);

            let sc_output = run_cmd(&["sc", "query", "shawl"]);
            let stdout = String::from_utf8_lossy(&sc_output.stdout);

            assert!(stdout.contains("STATE              : 1  STOPPED"));
            assert!(stdout.contains("WIN32_EXIT_CODE    : 0  (0x0)"));
        }

        it "reports a service-specific error for a failing command" {
            run_shawl(&["add", "--name", "shawl", "--", &child(), "--exit", "7"]);
            run_cmd(&["sc", "start", "shawl"]);
            run_cmd(&["sc", "stop", "shawl"]);

            let sc_output = run_cmd(&["sc", "query", "shawl"]);
            let stdout = String::from_utf8_lossy(&sc_output.stdout);

            assert!(stdout.contains("STATE              : 1  STOPPED"));
            assert!(stdout.contains("WIN32_EXIT_CODE    : 1066  (0x42a)"));
            assert!(stdout.contains("SERVICE_EXIT_CODE  : 7  (0x7)"));
        }

        it "handles a command that times out on stop" {
            run_shawl(&["add", "--name", "shawl", "--", &child(), "--infinite"]);
            run_cmd(&["sc", "start", "shawl"]);
            run_cmd(&["sc", "stop", "shawl"]);
            std::thread::sleep(std::time::Duration::from_secs(4));

            let sc_output = run_cmd(&["sc", "query", "shawl"]);
            let stdout = String::from_utf8_lossy(&sc_output.stdout);
            println!(">>>>>>> {}", stdout);

            assert!(stdout.contains("STATE              : 1  STOPPED"));
            assert!(stdout.contains("WIN32_EXIT_CODE    : 0  (0x0)"));
        }

        it "logs command output by default" {
            delete_log();

            run_shawl(&["add", "--name", "shawl", "--", &child()]);
            run_cmd(&["sc", "start", "shawl"]);
            run_cmd(&["sc", "stop", "shawl"]);

            let log = std::fs::read_to_string(log_file()).unwrap();
            assert!(log.contains("[shawl] stdout: \"shawl-child message on stdout\""));
            assert!(log.contains("[shawl] stderr: \"shawl-child message on stderr\""));
        }

        it "disables all logging with --no-log" {
            delete_log();

            run_shawl(&["add", "--name", "shawl", "--no-log", "--", &child()]);
            run_cmd(&["sc", "start", "shawl"]);
            run_cmd(&["sc", "stop", "shawl"]);

            assert!(!log_exists());
        }

        it "disables command logging with --no-log-cmd" {
            delete_log();

            run_shawl(&["add", "--name", "shawl", "--no-log-cmd", "--", &child()]);
            run_cmd(&["sc", "start", "shawl"]);
            run_cmd(&["sc", "stop", "shawl"]);

            let log = std::fs::read_to_string(log_file()).unwrap();
            assert!(!log.contains("shawl-child has started"));
        }
    }
}
