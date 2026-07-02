// main.rs - the REPL and -c flag entry point
//
// Supports two modes:
//   interactive: print a prompt, read a line, run it, repeat
//   batch:       minish -c "command line"  -- run once then exit

mod exec;
mod lexer;
mod parser;

use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // check for -c "cmd" batch mode
    if args.len() >= 3 && args[1] == "-c" {
        let line = args[2..].join(" ");
        let code = run_line(&line);
        std::process::exit(code);
    }

    // interactive REPL
    let stdin = io::stdin();
    loop {
        // print the prompt (flush so it appears before we block on read)
        print!("minish> ");
        io::stdout().flush().ok();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                // EOF (Ctrl-D): exit cleanly
                println!();
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("minish: read error: {}", e);
                break;
            }
        }

        let line = line.trim_end_matches('\n').trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }

        run_line(line);
        // ignore the exit code in interactive mode — the shell keeps running
    }
}

// Parse and execute one line of input.
// Returns the exit status of the command (used in -c mode).
fn run_line(line: &str) -> i32 {
    // step 1: lex
    let tokens = match lexer::tokenize(line) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("minish: {}", e);
            return 1;
        }
    };

    if tokens.is_empty() {
        return 0;
    }

    // step 2: parse
    let pipeline = match parser::parse(tokens) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("minish: {}", e);
            return 1;
        }
    };

    // step 3: execute
    exec::run_pipeline(pipeline)
}
