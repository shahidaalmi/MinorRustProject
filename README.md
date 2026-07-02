# minish — a tiny Unix shell in Rust

A small but real shell built with nothing but `std`. It runs external programs,
wires multi-stage pipelines concurrently, handles `<`/`>` I/O redirection, and
implements three builtins.

---

## Why can't `cd` be an external program?

Short answer: **every process has its own working directory, and a child process
cannot change its parent's working directory.**

When the shell runs an external command it `fork`s a child process (or on
Windows, creates a new process). That child is completely separate from the
shell. Even if you wrote a `/usr/bin/cd` that called `chdir("/tmp")`, it would
change the directory of *that child process*, which immediately exits. The
shell's own working directory never moves.

`cd` must be a **builtin** — code that runs *inside* the shell process itself —
so that when it calls `std::env::set_current_dir(...)` (which wraps the kernel's
`chdir` syscall), it changes the directory of the shell process that you're
actually typing into.

The same logic applies to `exit` (has to terminate the shell's own process) and
`pwd` (reads the shell's own working directory — trivial to make external, but
conventional to keep as a builtin for speed).

---

## Features

- Tokenizer with `"..."` / `'...'` quoting (operators inside quotes are literal)
- Multi-stage pipelines running **concurrently** (passes the `yes | head -3` test)
- `<` (stdin) and `>` (stdout, truncating) redirection
- Builtins: `cd`, `pwd`, `exit`
- Graceful errors: unterminated quotes, bad pipelines, missing commands — all
  print a message and the shell keeps running; no panics
- `-c "cmd"` batch mode for scripting / testing
- No zombie processes: every spawned child is waited on

## Out of scope (as per spec)

Signals, job control, `&&`/`||`/`;`, `$VAR` expansion, globbing, `>>` append.

---

## Project layout

```
src/
  main.rs    — REPL loop + -c mode
  lexer.rs   — pure tokenizer (unit-tested without spawning anything)
  parser.rs  — pure parser producing Pipeline/Cmd structs
  exec.rs    — only file that touches std::process
```

---

## Build & run

```bash
# build (release for a fast binary)
cargo build --release

# interactive mode
./target/release/minish

# batch mode
./target/release/minish -c "echo hello | tr a-z A-Z"

# run the unit tests (lexer + parser only, no spawning)
cargo test
```

---

## Acceptance test checklist

```bash
MINISH=./target/release/minish

# 1. whitespace collapsing
diff <($MINISH -c 'echo one two   three') <(bash -c 'echo one two   three')

# 2. quoting
diff <($MINISH -c "echo 'a  b' 'c  d'") <(bash -c "echo 'a  b' 'c  d'")

# 3. three-stage pipeline
diff <($MINISH -c "printf 'b\na\nc\n' | sort | head -2") \
     <(bash  -c "printf 'b\na\nc\n' | sort | head -2")

# 4. both redirections
$MINISH -c "echo data > /tmp/mt.txt"
diff <($MINISH -c "cat < /tmp/mt.txt") <(bash -c "cat < /tmp/mt.txt")

# 5. builtins mutate shell state (run interactively)
#    cd /tmp  then  pwd  → should print /tmp

# 6. concurrency: sequential shells hang here; minish should return immediately
$MINISH -c "yes | head -3"

# 7. missing command exits 127
$MINISH -c "nosuchcmd"; echo "exit: $?"

# 8. bad input — shell survives (interactive)
#    type:  "unclosed   then check prompt returns
#    type:  ls | | wc   then check prompt returns

# 9. no zombies
$MINISH -c "ls | wc -l"
ps aux | grep defunct   # should be empty
```

---

## Design notes

The project is split into three layers so each can be tested independently:

1. **`lexer.rs`** — pure function, takes a `&str`, returns `Vec<Token>` or an
   error string. No I/O. All unit tests here run without spawning a single
   process.

2. **`parser.rs`** — pure function, takes `Vec<Token>`, returns `Pipeline` or
   an error string. Same deal.

3. **`exec.rs`** — the only place `std::process` is touched. Builds
   `Command`s, plumbs pipes by moving `child.stdout` from one stage to the
   next, then waits on all children together.

The concurrency of pipelines comes for free: all stages are spawned before any
`wait` is called, so they run in parallel exactly as a real shell does.
