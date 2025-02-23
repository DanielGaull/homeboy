use std::error::Error;

use homeboy::templating::{matcher::TemplateMatcher, parser::TemplateParser};

#[test]
fn regex_generation_tests() -> Result<(), Box<dyn Error>> {
    let matcher = setup_matcher()?;

    assert_regex("foo", "^foo$", &matcher)?;
    assert_regex("foo?", "^(?:foo)?$", &matcher)?;
    assert_regex("(foo)?", "^(?:foo)?$", &matcher)?;
    assert_regex("[hello]", "^(?<hello>.*)$", &matcher)?;
    assert_regex("{pre command ask}?", r"^(?:(?:could|would)\s*you\s*(?:please)?)?$", &matcher)?;
    assert_regex(
        "{pre command ask}? play [song] on Spotify", 
        r"^(?:(?:could|would)\s*you\s*(?:please)?)?\s*play\s*(?<song>.*)\s*on\s*spotify$", 
        &matcher
    )?;

    Ok(())
}

#[test]
fn regex_matching_tests() -> Result<(), Box<dyn Error>> {
    let matcher = setup_matcher()?;

    assert_match("foo", "foo", vec![], &matcher)?;
    assert_match("{pre command ask}? foo", "foo", vec![], &matcher)?;
    assert_match("{pre command ask}? foo", "could you foo", vec![], &matcher)?;
    assert_match("{pre command ask}? foo", "could you please foo", vec![], &matcher)?;
    assert_match(
        "{pre command ask}? play [song] on Spotify", 
        "could you play enter sandman on spotify", 
        vec![("song", "enter sandman")],
        &matcher
    )?;

    assert_no_match("{pre command ask}? foo", "you please foo", &matcher)?;
    assert_no_match("{pre command ask}? foo", "baz", &matcher)?;
    assert_no_match("{pre command ask}? foo", "please baz", &matcher)?;
    assert_no_match("{pre command ask}? foo", "could you baz", &matcher)?;

    Ok(())
}

fn assert_regex(input_template: &str, expected_regex: &str, matcher: &TemplateMatcher) -> Result<(), Box<dyn Error>> {
    let template = TemplateParser::parse_template(input_template)?;
    let regex = matcher.convert_template_to_regex(&template)?;
    assert_eq!(expected_regex, regex.as_str());
    Ok(())
}

fn assert_match(input: &str, statement: &str, bindings: Vec<(&str, &str)>, matcher: &TemplateMatcher) -> Result<(), Box<dyn Error>> {
    let template = TemplateParser::parse_template(input)?;
    let matched = matcher.try_match(statement, &template)?.unwrap();
    assert_eq!(bindings.len(), matched.num_bindings());
    for b in bindings {
        let bound = matched.get_binding(b.0).unwrap();
        assert_eq!(b.1, bound);
    }
    Ok(())
}

fn assert_no_match(input: &str, statement: &str, matcher: &TemplateMatcher) -> Result<(), Box<dyn Error>> {
    let template = TemplateParser::parse_template(input)?;
    let matched = matcher.try_match(statement, &template)?;
    assert!(matched.is_none());
    Ok(())
}

fn setup_matcher() -> Result<TemplateMatcher, Box<dyn Error>> {
    let mut matcher = TemplateMatcher::new();

    let pre_command_ask = TemplateParser::parse_template("(could|would) you please?")?;
    matcher.add_subtemplate("pre command ask", pre_command_ask);
    Ok(matcher)
}
