use std::error::Error;

use homeboy::templating::{parser::TemplateParser, template::{Clause, Symbol, SymbolInternal, Template}};

#[test]
fn template_parsing_tests() -> Result<(), Box<dyn Error>> {
    run_test("hello", Template::single(Clause::single(Symbol::new(SymbolInternal::Text(String::from("hello")), false))))?;
    run_test("hello?", Template::single(Clause::single(Symbol::new(SymbolInternal::Text(String::from("hello")), true))))?;

    run_test("{hello}", Template::single(Clause::single(Symbol::new(SymbolInternal::SubtemplateCall(String::from("hello")), false))))?;
    run_test("{hello}?", Template::single(Clause::single(Symbol::new(SymbolInternal::SubtemplateCall(String::from("hello")), true))))?;

    run_test("[hello]", Template::single(Clause::single(Symbol::new(SymbolInternal::VarBind(String::from("hello")), false))))?;
    run_test("[hello]?", Template::single(Clause::single(Symbol::new(SymbolInternal::VarBind(String::from("hello")), true))))?;

    run_test("[hello]|hello|{hello}", Template::new(vec![
        Clause::single(Symbol::new(SymbolInternal::VarBind(String::from("hello")), false)),
        Clause::single(Symbol::new(SymbolInternal::Text(String::from("hello")), false)),
        Clause::single(Symbol::new(SymbolInternal::SubtemplateCall(String::from("hello")), false)),
    ]))?;

    run_test("foo bar baz", Template::single(Clause::new(vec![
        Symbol::new(SymbolInternal::Text(String::from("foo")), false),
        Symbol::new(SymbolInternal::Text(String::from("bar")), false),
        Symbol::new(SymbolInternal::Text(String::from("baz")), false),
    ])))?;

    run_test("(can|would)", Template::single(Clause::new(vec![
        Symbol::new(SymbolInternal::Template(
            Box::new(
                Template::new(vec![
                    Clause::single(Symbol::new(SymbolInternal::Text(String::from("can")), false)),
                    Clause::single(Symbol::new(SymbolInternal::Text(String::from("would")), false))
                ])
            )
        ), false),
    ])))?;

    run_test("{please} (can|would) you", Template::single(Clause::new(vec![
        Symbol::new(SymbolInternal::SubtemplateCall(String::from("please")), false),
        Symbol::new(SymbolInternal::Template(Box::new(Template::new(vec![
            Clause::single(Symbol::new(SymbolInternal::Text(String::from("can")), false)),
            Clause::single(Symbol::new(SymbolInternal::Text(String::from("would")), false)),
        ]))), false),
        Symbol::new(SymbolInternal::Text(String::from("you")), false),
    ])))?;

    Ok(())
}

fn run_test(input: &str, expected: Template) -> Result<(), Box<dyn Error>> {
    let template = TemplateParser::parse_template(input)?;
    assert_eq!(expected, template);
    Ok(())
}
