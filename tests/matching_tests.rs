use std::error::Error;

use homeboy::templating::{matcher::TemplateMatcher, parser::TemplateParser};

#[test]
fn regex_generation_tests() -> Result<(), Box<dyn Error>> {
    let mut matcher = TemplateMatcher::new();

    let pre_command_ask = TemplateParser::parse_template("(could|would) you please?")?;
    matcher.add_subtemplate("pre command ask", pre_command_ask);

    assert_regex("foo", "foo", &matcher)?;
    assert_regex("foo?", "foo?", &matcher)?;
    assert_regex("(foo)?", "(?:foo)?", &matcher)?;
    assert_regex("[hello]", "(?<hello>.*)", &matcher)?;
    assert_regex("{pre command ask}?", "(?:(?:could|would) you please?)?", &matcher)?;
    assert_regex(
        "{pre command ask}? play [song] on Spotify", 
        "(?:(?:could|would) you please?)? play (?<song>.*) on Spotify", 
        &matcher
    )?;

    Ok(())
}

fn assert_regex(input_template: &str, expected_regex: &str, matcher: &TemplateMatcher) -> Result<(), Box<dyn Error>> {
    let template = TemplateParser::parse_template(input_template)?;
    let regex = matcher.convert_template_to_regex(&template)?;
    assert_eq!(expected_regex, regex.as_str());
    Ok(())
}
