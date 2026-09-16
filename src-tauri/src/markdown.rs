// Read-only markdown rendering. Done in Rust (rather than shipping a JS
// markdown lib) so ticket 03's WYSIWYG editor can reuse the same parser.

use pulldown_cmark::{html, Options, Parser};

pub fn render(body: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(body, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_markdown_to_html() {
        let html = render("# Title\n\nSome *body* text.\n");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<em>body</em>"));
    }
}
