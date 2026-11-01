//! Parseur minimal du format texte KeyValues de Valve (fichiers .vdf / .acf de Steam).

#[derive(Debug, Clone)]
pub enum Vdf {
    Str(String),
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    /// Accès à une clé, insensible à la casse comme le fait Steam.
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(entries) => entries
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Obj(entries) => entries,
            Vdf::Str(_) => &[],
        }
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Vdf::as_str)
    }
}

enum Tok {
    Str(String),
    Open,
    Close,
}

pub fn parse(src: &str) -> Vdf {
    let tokens = tokenize(src);
    let mut pos = 0;
    Vdf::Obj(parse_obj(&tokens, &mut pos))
}

fn parse_obj(tokens: &[Tok], pos: &mut usize) -> Vec<(String, Vdf)> {
    let mut out = Vec::new();
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Tok::Close => {
                *pos += 1;
                break;
            }
            Tok::Open => {
                // Accolade sans clé : on l'ignore.
                *pos += 1;
            }
            Tok::Str(key) => {
                *pos += 1;
                match tokens.get(*pos) {
                    Some(Tok::Str(value)) => {
                        *pos += 1;
                        out.push((key.clone(), Vdf::Str(value.clone())));
                    }
                    Some(Tok::Open) => {
                        *pos += 1;
                        out.push((key.clone(), Vdf::Obj(parse_obj(tokens, pos))));
                    }
                    _ => break,
                }
            }
        }
    }
    out
}

fn tokenize(src: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut buf = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => match chars.next() {
                            Some('n') => buf.push('\n'),
                            Some('t') => buf.push('\t'),
                            Some(other) => buf.push(other),
                            None => break,
                        },
                        '"' => break,
                        _ => buf.push(c),
                    }
                }
                out.push(Tok::Str(buf));
            }
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {}
            c => {
                let mut buf = String::from(c);
                while let Some(&n) = chars.peek() {
                    if n.is_whitespace() || n == '{' || n == '}' || n == '"' {
                        break;
                    }
                    buf.push(n);
                    chars.next();
                }
                out.push(Tok::Str(buf));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_library_folders() {
        let src = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps"
		{
			"228980"		"312567"
		}
	}
}
"#;
        let root = parse(src);
        let lib = root.get("LibraryFolders").unwrap().get("0").unwrap();
        assert_eq!(lib.get_str("path"), Some(r"C:\Program Files (x86)\Steam"));
        assert_eq!(lib.get("apps").unwrap().entries().len(), 1);
    }
}
