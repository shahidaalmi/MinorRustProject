// lexer.rs - breaks a raw command line into a list of tokens
// This is a pure function: no I/O, no side effects, easy to unit test.

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Word(String), // a regular word or quoted string
    Pipe,         // |
    RedirectIn,   // <
    RedirectOut,  // >
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            // skip plain whitespace between tokens
            ' ' | '\t' => {
                chars.next();
            }

            // pipe operator
            '|' => {
                chars.next();
                tokens.push(Token::Pipe);
            }

            // redirect stdin
            '<' => {
                chars.next();
                tokens.push(Token::RedirectIn);
            }

            // redirect stdout
            '>' => {
                chars.next();
                tokens.push(Token::RedirectOut);
            }

            // double-quoted string: contents are one word, operators inside are literal
            '"' => {
                chars.next(); // consume the opening quote
                let mut word = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    if c == '"' {
                        closed = true;
                        break;
                    }
                    word.push(c);
                }
                if !closed {
                    return Err("unterminated double quote".to_string());
                }
                tokens.push(Token::Word(word));
            }

            // single-quoted string: same behavior as double quotes for our purposes
            '\'' => {
                chars.next(); // consume opening quote
                let mut word = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    if c == '\'' {
                        closed = true;
                        break;
                    }
                    word.push(c);
                }
                if !closed {
                    return Err("unterminated single quote".to_string());
                }
                tokens.push(Token::Word(word));
            }

            // anything else: collect until whitespace or an operator
            _ => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ' ' || c == '\t' || c == '|' || c == '<' || c == '>' {
                        break;
                    }
                    word.push(c);
                    chars.next();
                }
                tokens.push(Token::Word(word));
            }
        }
    }

    Ok(tokens)
}

// ---- unit tests ----
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_words() {
        let t = tokenize("ls -l").unwrap();
        assert_eq!(t, vec![Token::Word("ls".into()), Token::Word("-l".into())]);
    }

    #[test]
    fn double_quote_groups_spaces() {
        let t = tokenize(r#"echo "a  b""#).unwrap();
        assert_eq!(
            t,
            vec![Token::Word("echo".into()), Token::Word("a  b".into())]
        );
    }

    #[test]
    fn single_quote_groups_spaces() {
        let t = tokenize("echo 'c  d'").unwrap();
        assert_eq!(
            t,
            vec![Token::Word("echo".into()), Token::Word("c  d".into())]
        );
    }

    #[test]
    fn pipe_and_redirect() {
        let t = tokenize("cat < in.txt | wc > out.txt").unwrap();
        assert_eq!(
            t,
            vec![
                Token::Word("cat".into()),
                Token::RedirectIn,
                Token::Word("in.txt".into()),
                Token::Pipe,
                Token::Word("wc".into()),
                Token::RedirectOut,
                Token::Word("out.txt".into()),
            ]
        );
    }

    #[test]
    fn operator_inside_quotes_is_literal() {
        let t = tokenize("echo '|'").unwrap();
        assert_eq!(
            t,
            vec![Token::Word("echo".into()), Token::Word("|".into())]
        );
    }

    #[test]
    fn unterminated_double_quote_is_error() {
        assert!(tokenize(r#"echo "oops"#).is_err());
    }

    #[test]
    fn unterminated_single_quote_is_error() {
        assert!(tokenize("echo 'oops").is_err());
    }
}
