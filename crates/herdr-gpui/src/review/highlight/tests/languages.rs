use super::*;
use test_case::test_case;

/// The text and token of each span `colour_file` gives `line`, the only
/// line of a file named `name`.
fn file_tokens(name: &str, line: &str) -> Vec<(String, Token)> {
    colour_file(name, line, 1)
        .first()
        .into_iter()
        .flatten()
        .map(|span| (line[span.start..span.end].to_owned(), span.token))
        .collect()
}

#[test_case("devMode.ts", "function setDevFlag(on: boolean): void {}"; "typescript")]
#[test_case("menu.tsx", "function Menu() { return <div className=\"x\" />; }"; "tsx")]
#[test_case("App.jsx", "function App() { return <div />; }"; "jsx")]
#[test_case("jest.config.js", "function config() {}"; "javascript")]
#[test_case("server.mjs", "function serve() {}"; "module javascript")]
#[test_case("server.cjs", "function serve() {}"; "commonjs")]
fn script_files_colour_keywords_and_functions(name: &str, line: &str) {
    let tokens = file_tokens(name, line);
    assert!(
        tokens.contains(&("function".into(), Token::Keyword)),
        "{tokens:?}"
    );
    assert!(
        tokens.iter().any(|(_, token)| *token == Token::Function),
        "{tokens:?}"
    );
}

#[test]
fn typescript_types_and_strings_are_coloured() {
    let tokens = file_tokens("devMode.ts", "const KEY: string = 'lanzarote_dev_mode';");
    assert!(
        tokens.contains(&("const".into(), Token::Keyword)),
        "{tokens:?}"
    );
    assert!(
        tokens.contains(&("string".into(), Token::Type)),
        "{tokens:?}"
    );
    assert!(
        tokens
            .iter()
            .any(|(text, token)| text.contains("lanzarote") && *token == Token::String),
        "{tokens:?}"
    );
}

#[test_case("index.html", "<div class=\"card\"></div>", "div", Token::Type; "html tag")]
#[test_case("index.css", ".card { color: #fff; }", "#fff", Token::Constant; "css colour")]
#[test_case("theme.scss", "$gap: 4px;", "4", Token::Number; "scss number")]
#[test_case("Cargo.toml", "name = \"herdr\"", "\"herdr\"", Token::String; "toml string")]
fn markup_and_config_files_are_coloured(name: &str, line: &str, text: &str, token: Token) {
    let tokens = file_tokens(name, line);
    assert!(
        tokens
            .iter()
            .any(|(span, found)| span.contains(text.trim_matches('"')) && *found == token),
        "{tokens:?}"
    );
}

#[test]
fn files_named_without_an_extension_find_their_grammar() {
    assert!(!file_tokens("Dockerfile", "FROM alpine").is_empty());
}

#[test]
fn a_shebang_names_the_grammar_of_a_file_without_an_extension() {
    let text = "#!/bin/bash\n# a comment\n";
    let lines = colour_file("deploy", text, 2);
    assert!(
        lines[1].iter().any(|span| span.token == Token::Comment),
        "{lines:?}"
    );
}
