//! `Exec` lines of desktop entries: string unescaping, quoting, and the
//! `%f %F %u %U %i %c %k` field codes.

use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

use crate::util::uri::glib_file_uri;

const ICON_FLAG: &str = "--icon";

/// What the non-file field codes expand to.
#[derive(Debug, Clone, Copy)]
pub struct ExecContext<'a> {
    pub name: &'a str,
    pub icon: Option<&'a str>,
    pub desktop_file: &'a Path,
}

/// The command lines that open `files`: one when the entry takes a list
/// (`%F`, `%U`), one per file when it takes a single one (`%f`, `%u`). An
/// entry with no file code gets the paths appended.
pub fn expand_exec(exec: &str, files: &[&Path], context: &ExecContext<'_>) -> Vec<Vec<OsString>> {
    let args = split_exec(&unescape_value(exec));
    if args.is_empty() {
        return Vec::new();
    }
    let codes = || args.iter().flat_map(|arg| field_codes(arg));
    let takes_list = codes().any(|code| matches!(code, 'F' | 'U'));
    let takes_one = codes().any(|code| matches!(code, 'f' | 'u'));
    let appends = !takes_list && !takes_one;
    if takes_one && !takes_list && files.len() > 1 {
        return files
            .iter()
            .map(|file| expand_args(&args, std::slice::from_ref(file), context, false))
            .collect();
    }
    vec![expand_args(&args, files, context, appends)]
}

fn expand_args(
    args: &[String],
    files: &[&Path],
    context: &ExecContext<'_>,
    append: bool,
) -> Vec<OsString> {
    let mut argv = Vec::with_capacity(args.len() + files.len());
    for arg in args {
        match arg.as_str() {
            "%F" => argv.extend(files.iter().map(|file| file.as_os_str().to_owned())),
            "%U" => argv.extend(files.iter().map(|file| OsString::from(glib_file_uri(file)))),
            "%i" => {
                if let Some(icon) = context.icon {
                    argv.extend([OsString::from(ICON_FLAG), OsString::from(icon)]);
                }
            }
            _ => {
                let expanded = expand_inline(arg, files.first().copied(), context);
                if !expanded.is_empty() || !is_lone_code(arg) {
                    argv.push(expanded);
                }
            }
        }
    }
    if append {
        argv.extend(files.iter().map(|file| file.as_os_str().to_owned()));
    }
    argv
}

fn expand_inline(arg: &str, file: Option<&Path>, context: &ExecContext<'_>) -> OsString {
    let mut expanded = OsString::with_capacity(arg.len());
    let mut chars = arg.chars();
    while let Some(character) = chars.next() {
        if character != '%' {
            expanded.push(character.encode_utf8(&mut [0; 4]));
            continue;
        }
        match chars.next() {
            Some('%') => expanded.push("%"),
            Some('f' | 'F') => expanded.push(file.map_or(OsStr::new(""), Path::as_os_str)),
            Some('u' | 'U') => expanded.push(file.map(glib_file_uri).unwrap_or_default()),
            Some('c') => expanded.push(context.name),
            Some('k') => expanded.push(context.desktop_file),
            Some('i') => expanded.push(context.icon.unwrap_or_default()),
            Some(_) => {}
            None => expanded.push("%"),
        }
    }
    expanded
}

/// Whether `arg` is a single field code, which leaves no argument behind
/// when it expands to nothing.
fn is_lone_code(arg: &str) -> bool {
    arg.len() == 2 && arg.starts_with('%') && arg != "%%"
}

/// The codes in `arg`, `%%` excluded.
fn field_codes(arg: &str) -> impl Iterator<Item = char> + '_ {
    let mut chars = arg.chars();
    std::iter::from_fn(move || {
        loop {
            if chars.next()? != '%' {
                continue;
            }
            match chars.next()? {
                '%' => continue,
                code => return Some(code),
            }
        }
    })
}

/// Undoes the string escapes every desktop-entry value may carry
/// (`\s \n \t \r \\`), leaving other backslashes to the `Exec` quoting.
pub fn unescape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Splits a command line on whitespace, honouring the quoting the desktop
/// entry spec allows.
pub fn split_exec(exec: &str) -> Vec<String> {
    let mut argv = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut quoted_empty = false;
    for character in exec.chars() {
        if escaped {
            current.push(character);
            escaped = false;
        } else if character == '\\' && quote == Some('"') {
            escaped = true;
        } else if Some(character) == quote {
            quote = None;
            quoted_empty = current.is_empty();
        } else if quote.is_none() && (character == '"' || character == '\'') {
            quote = Some(character);
        } else if quote.is_none() && character.is_whitespace() {
            if !current.is_empty() || quoted_empty {
                argv.push(std::mem::take(&mut current));
            }
            quoted_empty = false;
        } else {
            current.push(character);
            quoted_empty = false;
        }
    }
    if !current.is_empty() || quoted_empty {
        argv.push(current);
    }
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(exec: &str, files: &[&str]) -> Vec<Vec<String>> {
        let context = ExecContext {
            name: "Text Editor",
            icon: Some("accessories-text-editor"),
            desktop_file: Path::new("/usr/share/applications/editor.desktop"),
        };
        let files: Vec<&Path> = files.iter().map(Path::new).collect();
        expand_exec(exec, &files, &context)
            .into_iter()
            .map(|argv| {
                argv.into_iter()
                    .map(|arg| arg.into_string().unwrap())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn lists_take_every_file_in_one_command() {
        assert_eq!(
            expand("gimp %F", &["/a b.png", "/c.png"]),
            [vec!["gimp", "/a b.png", "/c.png"]]
        );
        assert_eq!(
            expand("browser %U", &["/a b.html"]),
            [vec!["browser", "file:///a%20b.html"]]
        );
    }

    #[test]
    fn single_file_codes_run_once_per_file() {
        assert_eq!(
            expand("editor --open=%f", &["/a", "/b"]),
            [vec!["editor", "--open=/a"], vec!["editor", "--open=/b"]]
        );
        assert_eq!(expand("player %u", &[]), [vec!["player"]]);
    }

    #[test]
    fn icon_name_and_location_codes() {
        assert_eq!(
            expand("editor %i --class=%c %k %%", &[]),
            [vec![
                "editor",
                "--icon",
                "accessories-text-editor",
                "--class=Text Editor",
                "/usr/share/applications/editor.desktop",
                "%",
            ]]
        );
    }

    #[test]
    fn an_entry_without_file_codes_gets_the_paths_appended() {
        assert_eq!(
            expand("viewer --new", &["/x"]),
            [vec!["viewer", "--new", "/x"]]
        );
    }

    #[test]
    fn deprecated_codes_vanish_and_quotes_hold_arguments() {
        assert_eq!(
            expand(r#""/opt/My App/run" %d --title="a \"b\"" %f"#, &["/f"]),
            [vec!["/opt/My App/run", "--title=a \"b\"", "/f"]]
        );
    }

    #[test]
    fn string_escapes_are_undone_before_splitting() {
        assert_eq!(unescape_value(r"a\sb\\c\n"), "a b\\c\n");
        assert_eq!(
            split_exec(&unescape_value(r#"sh -c "echo \\"hi\\"""#)),
            ["sh", "-c", "echo \"hi\""]
        );
        assert!(split_exec("   ").is_empty());
        assert_eq!(split_exec("app ''"), ["app", ""]);
    }
}
