//! Valve's text KeyValues format (libraryfolders.vdf, appmanifest_*.acf).

#[derive(Debug, Clone, PartialEq)]
pub enum Vdf {
    Str(String),
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    /// Case-insensitive, like Steam itself.
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(pairs) => pairs.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Obj(pairs) => pairs,
            Vdf::Str(_) => &[],
        }
    }
}

enum Token {
    Text(String),
    Open,
    Close,
}

fn tokens(src: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '/' => {
                // Comments run to the end of the line.
                while chars.next().is_some_and(|c| c != '\n') {}
            }
            '{' => {
                chars.next();
                out.push(Token::Open);
            }
            '}' => {
                chars.next();
                out.push(Token::Close);
            }
            '[' => {
                // Platform conditionals such as [$WIN32] are ignored.
                while chars.next().is_some_and(|c| c != ']') {}
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => break,
                        },
                        _ => s.push(c),
                    }
                }
                out.push(Token::Text(s));
            }
            _ => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || matches!(c, '{' | '}' | '"') {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                out.push(Token::Text(s));
            }
        }
    }
    out
}

fn object(tokens: &mut std::vec::IntoIter<Token>) -> Vec<(String, Vdf)> {
    let mut pairs = Vec::new();
    while let Some(token) = tokens.next() {
        let key = match token {
            Token::Text(k) => k,
            Token::Close => break,
            Token::Open => continue,
        };
        match tokens.next() {
            Some(Token::Text(v)) => pairs.push((key, Vdf::Str(v))),
            Some(Token::Open) => pairs.push((key, Vdf::Obj(object(tokens)))),
            Some(Token::Close) | None => break,
        }
    }
    pairs
}

/// The whole file as one object of top-level pairs.
pub fn parse(src: &str) -> Vdf {
    Vdf::Obj(object(&mut tokens(src).into_iter()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_library_folders() {
        let src = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "228980" "1234" }
	}
	"1" { "path" "D:\\SteamLibrary" } // second drive
}"#;
        let root = parse(src);
        let folders = root.get("LibraryFolders").unwrap();
        let paths: Vec<_> = folders.entries().iter().filter_map(|(_, f)| f.str("path")).collect();
        assert_eq!(paths, ["C:\\Program Files (x86)\\Steam", "D:\\SteamLibrary"]);
    }

    #[test]
    fn reads_an_app_manifest_with_conditionals() {
        let src = "\"AppState\"\n{\n\"appid\" \"620\"\n\"name\" \"Portal 2\"\n\"StateFlags\" \"4\" [$WIN32]\n}";
        let app = parse(src);
        let state = app.get("appstate").unwrap();
        assert_eq!(state.str("appid"), Some("620"));
        assert_eq!(state.str("name"), Some("Portal 2"));
        assert_eq!(state.str("stateflags"), Some("4"));
    }

    #[test]
    fn survives_broken_input() {
        assert_eq!(parse("\"a\" { \"b\" "), Vdf::Obj(vec![("a".into(), Vdf::Obj(vec![]))]));
        assert_eq!(parse(""), Vdf::Obj(vec![]));
    }
}
