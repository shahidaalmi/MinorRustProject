// parser.rs - turns a flat list of tokens into a Pipeline struct.
// Still a pure function: nothing is spawned here.

use crate::lexer::Token;

// One command in a pipeline, e.g.  grep ".rs" < input.txt
#[derive(Debug, PartialEq)]
pub struct Cmd {
    pub argv: Vec<String>,          // argv[0] is the program name
    pub stdin_from: Option<String>, // < filename
    pub stdout_to: Option<String>,  // > filename
}

// A pipeline is one or more commands joined by |
#[derive(Debug, PartialEq)]
pub struct Pipeline {
    pub commands: Vec<Cmd>,
}

pub fn parse(tokens: Vec<Token>) -> Result<Pipeline, String> {
    let mut commands: Vec<Cmd> = Vec::new();

    // current command being assembled
    let mut argv: Vec<String> = Vec::new();
    let mut stdin_from: Option<String> = None;
    let mut stdout_to: Option<String> = None;

    let mut iter = tokens.into_iter().peekable();

    while let Some(tok) = iter.next() {
        match tok {
            Token::Word(w) => {
                argv.push(w);
            }

            Token::RedirectIn => {
                // next token must be a filename
                match iter.next() {
                    Some(Token::Word(file)) => {
                        stdin_from = Some(file);
                    }
                    _ => return Err("expected filename after '<'".to_string()),
                }
            }

            Token::RedirectOut => {
                match iter.next() {
                    Some(Token::Word(file)) => {
                        stdout_to = Some(file);
                    }
                    _ => return Err("expected filename after '>'".to_string()),
                }
            }

            Token::Pipe => {
                // finish the current command and start a new one
                if argv.is_empty() {
                    return Err("unexpected '|': no command before pipe".to_string());
                }
                commands.push(Cmd {
                    argv,
                    stdin_from,
                    stdout_to,
                });
                argv = Vec::new();
                stdin_from = None;
                stdout_to = None;

                // a pipe must be followed by something
                if iter.peek().is_none() {
                    return Err("unexpected '|': no command after pipe".to_string());
                }
            }
        }
    }

    // push whatever we were building as the final command
    if argv.is_empty() {
        // the whole input was empty (or only operators)
        if commands.is_empty() {
            return Err("empty command".to_string());
        }
        return Err("trailing operator without command".to_string());
    }

    commands.push(Cmd {
        argv,
        stdin_from,
        stdout_to,
    });

    Ok(Pipeline { commands })
}

// ---- unit tests ----
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn p(input: &str) -> Result<Pipeline, String> {
        tokenize(input).and_then(parse)
    }

    #[test]
    fn simple_command() {
        let pl = p("ls -l").unwrap();
        assert_eq!(pl.commands.len(), 1);
        assert_eq!(pl.commands[0].argv, vec!["ls", "-l"]);
    }

    #[test]
    fn two_stage_pipeline() {
        let pl = p("ls | wc").unwrap();
        assert_eq!(pl.commands.len(), 2);
        assert_eq!(pl.commands[0].argv[0], "ls");
        assert_eq!(pl.commands[1].argv[0], "wc");
    }

    #[test]
    fn redirections_parsed() {
        let pl = p("sort < in.txt > out.txt").unwrap();
        assert_eq!(pl.commands[0].stdin_from, Some("in.txt".into()));
        assert_eq!(pl.commands[0].stdout_to, Some("out.txt".into()));
    }

    #[test]
    fn double_pipe_is_error() {
        assert!(p("ls | | wc").is_err());
    }

    #[test]
    fn trailing_pipe_is_error() {
        assert!(p("ls |").is_err());
    }

    #[test]
    fn leading_pipe_is_error() {
        assert!(p("| ls").is_err());
    }
}
