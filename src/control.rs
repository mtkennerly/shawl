use crate::cli::CommonOpts;
use log::error;
use std::io::Write;

pub fn add_service(
    name: String,
    cwd: Option<String>,
    dependencies: &[String],
    opts: CommonOpts,
) -> Result<(), ()> {
    let shawl_path = quote(
        &std::env::current_exe()
            .expect("Unable to determine Shawl location")
            .to_string_lossy(),
    );
    let shawl_args = construct_shawl_run_args(&name, &cwd, &opts);
    let prepared_command = prepare_command(&opts.command);

    let mut cmd = std::process::Command::new("sc");
    cmd.arg("create").arg(&name);

    if !dependencies.is_empty() {
        cmd.arg("depend=");
        cmd.arg(quote(&dependencies.join("/")));
    }

    let output = cmd
        .arg("binPath=")
        .arg(format!(
            "{} {} -- {}",
            shawl_path,
            shawl_args.join(" "),
            prepared_command.join(" ")
        ))
        .output()
        .expect("Failed to create the service");
    match output.status.code() {
        Some(0) => Ok(()),
        Some(x) => {
            error!("Failed to create the service. Error code: {}.", x);
            error!("SC stdout:\n{}", String::from_utf8_lossy(&output.stdout));
            error!("SC stderr:\n{}", String::from_utf8_lossy(&output.stderr));
            Err(())
        }
        None => {
            error!("Failed to create the service. Output:");
            std::io::stderr().write_all(&output.stdout).unwrap();
            std::io::stderr().write_all(&output.stderr).unwrap();
            Err(())
        }
    }
}

fn construct_shawl_run_args(name: &str, cwd: &Option<String>, opts: &CommonOpts) -> Vec<String> {
    let mut shawl_args = vec!["run".to_string(), "--name".to_string(), quote(name)];
    if let Some(st) = opts.stop_timeout {
        shawl_args.push("--stop-timeout".to_string());
        shawl_args.push(st.to_string());
    }
    if opts.restart {
        shawl_args.push("--restart".to_string());
    }
    if opts.no_restart {
        shawl_args.push("--no-restart".to_string());
    }
    if !opts.restart_if.is_empty() {
        shawl_args.push("--restart-if".to_string());
        shawl_args.push(
            opts.restart_if
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(","),
        );
    }
    if !opts.restart_if_not.is_empty() {
        shawl_args.push("--restart-if-not".to_string());
        shawl_args.push(
            opts.restart_if_not
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(","),
        );
    };
    if let Some(pass) = &opts.pass {
        shawl_args.push("--pass".to_string());
        shawl_args.push(
            pass.iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(","),
        );
    }
    if let Some(cwd) = &cwd {
        shawl_args.push("--cwd".to_string());
        shawl_args.push(quote(cwd));
    };
    if opts.no_log {
        shawl_args.push("--no-log".to_string());
    }
    if opts.no_log_cmd {
        shawl_args.push("--no-log-cmd".to_string());
    }
    if let Some(log_dir) = &opts.log_dir {
        shawl_args.push("--log-dir".to_string());
        shawl_args.push(quote(log_dir));
    }
    if opts.pass_start_args {
        shawl_args.push("--pass-start-args".to_string());
    }
    if !opts.env.is_empty() {
        for (x, y) in &opts.env {
            shawl_args.push("--env".to_string());
            shawl_args.push(quote(&format!("{}={}", x, y)));
        }
    }
    if !opts.path.is_empty() {
        for path in &opts.path {
            shawl_args.push("--path".to_string());
            shawl_args.push(quote(path));
        }
    }
    if let Some(priority) = opts.priority {
        shawl_args.push("--priority".to_string());
        shawl_args.push(priority.to_cli());
    }
    shawl_args
}

fn prepare_command(command: &[String]) -> Vec<String> {
    command.iter().map(|x| quote(x)).collect::<Vec<String>>()
}

fn quote(text: &str) -> String {
    if text.contains(' ') {
        format!("\"{}\"", text)
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
speculate::speculate! {
    fn s(text: &str) -> String {
        text.to_string()
    }

    describe "construct_shawl_run_args" {
        it "works with minimal input" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts::default()
                ),
                vec!["run", "--name", "shawl"],
            );
        }

        it "does not use the command" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        command: vec![s("foo")],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl"],
            );
        }

        it "handles --name with spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("C:/Program Files/shawl"),
                    &None,
                    &CommonOpts::default()
                ),
                vec!["run", "--name", "\"C:/Program Files/shawl\""],
            );
        }

        it "handles --restart" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        restart: true,
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--restart"],
            );
        }

        it "handles --no-restart" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        no_restart: true,
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--no-restart"],
            );
        }

        it "handles --restart-if with one code" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        restart_if: vec![0],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--restart-if", "0"],
            );
        }

        it "handles --restart-if with multiple codes" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        restart_if: vec![1, 10],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--restart-if", "1,10"],
            );
        }

        it "handles --restart-if-not with one code" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        restart_if_not: vec![0],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--restart-if-not", "0"],
            );
        }

        it "handles --restart-if-not with multiple codes" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        restart_if_not: vec![1, 10],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--restart-if-not", "1,10"],
            );
        }

        it "handles --pass with one code" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: Some(vec![0]),
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--pass", "0"],
            );
        }

        it "handles --pass with multiple codes" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: Some(vec![1, 10]),
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--pass", "1,10"],
            );
        }

        it "handles --stop-timeout" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        stop_timeout: Some(3000),
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000"],
            );
        }

        it "handles --cwd without spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &Some(s("C:/foo")),
                    &CommonOpts::default()
                ),
                vec!["run", "--name", "shawl", "--cwd", "C:/foo"],
            );
        }

        it "handles --cwd with spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &Some(s("C:/Program Files/foo")),
                    &CommonOpts::default()
                ),
                vec!["run", "--name", "shawl", "--cwd", "\"C:/Program Files/foo\""],
            );
        }

        it "handles --no-log" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        no_log: true,
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--no-log"],
            );
        }
        it "handles --no-log-cmd" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        no_log_cmd: true,
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--no-log-cmd"],
            );
        }

        it "handles --log-dir without spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        log_dir: Some("C:/foo".to_string()),
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--log-dir", "C:/foo"],
            );
        }

        it "handles --log-dir with spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        log_dir: Some("C:/foo bar/hello".to_string()),
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--log-dir", "\"C:/foo bar/hello\""],
            );
        }

        it "handles --pass-start-args" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass_start_args: true,
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--pass-start-args"],
            );
        }

        it "handles --env without spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        env: vec![(s("FOO"), s("bar"))],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--env", "FOO=bar"],
            );
        }

        it "handles --env with spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        env: vec![(s("FOO"), s("bar baz"))],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--env", "\"FOO=bar baz\""],
            );
        }

        it "handles --env multiple times" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        env: vec![(s("FOO"), s("1")), (s("BAR"), s("2"))],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--env", "FOO=1", "--env", "BAR=2"],
            );
        }

        it "handles --path without spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        path: vec![s("C:/foo")],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--path", "C:/foo"],
            );
        }

        it "handles --path with spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        path: vec![s("C:/foo bar")],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--path", "\"C:/foo bar\""],
            );
        }

        it "handles --path multiple times" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        path: vec![s("C:/foo"), s("C:/bar")],
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--path", "C:/foo", "--path", "C:/bar"],
            );
        }

        it "handles --priority" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        priority: Some(crate::cli::Priority::AboveNormal),
                        ..Default::default()
                    }
                ),
                vec!["run", "--name", "shawl", "--priority", "above-normal"],
            );
        }
    }

    describe "prepare_command" {
        it "handles commands without inner spaces" {
            assert_eq!(
                prepare_command(&[s("cat"), s("file")]),
                vec![s("cat"), s("file")],
            );
        }

        it "handles commands with inner spaces" {
            assert_eq!(
                prepare_command(&[s("cat"), s("some file")]),
                vec![s("cat"), s("\"some file\"")],
            );
        }
    }
}
