// exec.rs - the ONLY module that touches std::process.
// Takes a Pipeline from the parser and runs it.

use std::fs::{File, OpenOptions};
use std::process::{Child, Command, Stdio};

use crate::parser::{Cmd, Pipeline};

// Run a pipeline and return the exit status of its last command.
// Returns None if there was nothing to run (empty pipeline guard).
pub fn run_pipeline(pipeline: Pipeline) -> i32 {
    let cmds = pipeline.commands;
    let n = cmds.len();

    // single command — handle builtins and simple external
    if n == 1 {
        return run_single(cmds.into_iter().next().unwrap());
    }

    // multi-stage pipeline — spawn all concurrently, then wait
    run_multi(cmds)
}

// ---- builtins ----

// Returns Some(exit_code) if the argv[0] is a builtin, None otherwise.
fn try_builtin(argv: &[String]) -> Option<i32> {
    match argv[0].as_str() {
        "exit" => {
            // exit [code]
            let code = argv.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
            std::process::exit(code);
        }

        "cd" => {
            let target = if let Some(dir) = argv.get(1) {
                std::path::PathBuf::from(dir)
            } else {
                // no arg: go home
                match std::env::var("HOME") {
                    Ok(h) => std::path::PathBuf::from(h),
                    Err(_) => {
                        eprintln!("minish: cd: HOME not set");
                        return Some(1);
                    }
                }
            };

            if let Err(e) = std::env::set_current_dir(&target) {
                eprintln!("minish: cd: {}: {}", target.display(), e);
                return Some(1);
            }
            Some(0)
        }

        "pwd" => {
            match std::env::current_dir() {
                Ok(p) => println!("{}", p.display()),
                Err(e) => {
                    eprintln!("minish: pwd: {}", e);
                    return Some(1);
                }
            }
            Some(0)
        }

        _ => None,
    }
}

// ---- single command (with optional redirects) ----

fn run_single(cmd: Cmd) -> i32 {
    // builtins go first
    if let Some(code) = try_builtin(&cmd.argv) {
        return code;
    }

    // open stdin redirect file if requested
    let stdin_file: Option<File> = match &cmd.stdin_from {
        Some(path) => match File::open(path) {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("minish: {}: {}", path, e);
                return 1;
            }
        },
        None => None,
    };

    // open stdout redirect file if requested (truncates)
    let stdout_file: Option<File> = match &cmd.stdout_to {
        Some(path) => match OpenOptions::new().write(true).create(true).truncate(true).open(path) {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("minish: {}: {}", path, e);
                return 1;
            }
        },
        None => None,
    };

    let mut builder = Command::new(&cmd.argv[0]);
    builder.args(&cmd.argv[1..]);

    if let Some(f) = stdin_file {
        builder.stdin(Stdio::from(f));
    }
    if let Some(f) = stdout_file {
        builder.stdout(Stdio::from(f));
    }

    match builder.spawn() {
        Ok(mut child) => wait_child(&mut child),
        Err(_) => {
            eprintln!("minish: command not found: {}", cmd.argv[0]);
            127
        }
    }
}

// ---- multi-stage pipeline ----

fn run_multi(cmds: Vec<Cmd>) -> i32 {
    let n = cmds.len();
    let mut children: Vec<Child> = Vec::with_capacity(n);

    // prev_stdout carries the read-end of the previous stage's pipe
    let mut prev_stdout: Option<std::process::ChildStdout> = None;

    for (i, cmd) in cmds.into_iter().enumerate() {
        let is_last = i == n - 1;

        // builtins cannot sensibly participate in a pipeline in this implementation
        // (they'd need to run in a subprocess), so we treat them as external and
        // they'll fail with "command not found" — acceptable for this project scope.

        // set up stdin: either the previous pipe or a < redirect
        let stdin_stdio: Stdio = if let Some(prev) = prev_stdout.take() {
            Stdio::from(prev)
        } else if let Some(ref path) = cmd.stdin_from {
            match File::open(path) {
                Ok(f) => Stdio::from(f),
                Err(e) => {
                    eprintln!("minish: {}: {}", path, e);
                    // reap already-spawned children before returning
                    reap_all(&mut children);
                    return 1;
                }
            }
        } else {
            Stdio::inherit()
        };

        // set up stdout: pipe to next stage unless this is the last command
        let stdout_stdio: Stdio = if !is_last {
            Stdio::piped()
        } else if let Some(ref path) = cmd.stdout_to {
            match OpenOptions::new().write(true).create(true).truncate(true).open(path) {
                Ok(f) => Stdio::from(f),
                Err(e) => {
                    eprintln!("minish: {}: {}", path, e);
                    reap_all(&mut children);
                    return 1;
                }
            }
        } else {
            Stdio::inherit()
        };

        let mut builder = Command::new(&cmd.argv[0]);
        builder.args(&cmd.argv[1..]);
        builder.stdin(stdin_stdio);
        builder.stdout(stdout_stdio);

        match builder.spawn() {
            Ok(mut child) => {
                // take the stdout pipe so the next stage can read from it
                if !is_last {
                    prev_stdout = child.stdout.take();
                }
                children.push(child);
            }
            Err(_) => {
                eprintln!("minish: command not found: {}", cmd.argv[0]);
                // drop prev_stdout so the previous stage gets a broken pipe
                drop(prev_stdout);
                reap_all(&mut children);
                return 127;
            }
        }
    }

    // wait on ALL children; status of the pipeline = last child's status
    let last_status = reap_all(&mut children);
    last_status
}

// Wait for every child in order and return the status of the last one.
// This ensures no zombie processes are left behind.
fn reap_all(children: &mut Vec<Child>) -> i32 {
    let mut last = 0i32;
    for child in children.iter_mut() {
        match child.wait() {
            Ok(status) => {
                last = status.code().unwrap_or(1);
            }
            Err(e) => {
                eprintln!("minish: wait error: {}", e);
                last = 1;
            }
        }
    }
    last
}

fn wait_child(child: &mut Child) -> i32 {
    match child.wait() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("minish: wait error: {}", e);
            1
        }
    }
}
