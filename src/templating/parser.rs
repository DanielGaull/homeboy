use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
use thiserror::Error;

use super::template::{Symbol, SymbolInternal, Template};

#[derive(Parser)]
#[grammar = "templating/grammar.pest"] // relative to src
struct PestTemplateParser;

pub struct TemplateParser;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Failed to parse template: {0}")]
    FailTemplate(String),
    #[error("Failed to parse symbol: {0}")]
    FailSymbol(String),
}

impl TemplateParser {
    pub fn parse_template(input: &str) -> Result<Template, ParseError> {
        let pair = PestTemplateParser::parse(Rule::template, input);
        match pair {
            Ok(mut v) => Self::parse_template_pair(v.next().unwrap()),
            Err(_) => Err(ParseError::FailTemplate(String::from(input))),
        }
    }

    fn parse_template_pair(pair: Pair<Rule>) -> Result<Template, ParseError> {
        let mut symbols = Vec::new();
        let pairs = pair.into_inner();
        for p in pairs {
            symbols.push(Self::parse_symbol(p)?);
        }
        Ok(Template { symbols: symbols })
    }

    fn parse_symbol(pair: Pair<Rule>) -> Result<Symbol, ParseError> {
        let pair_str = pair.as_str();
        let symbol_internal;
        let internal_pair = pair.into_inner().next().unwrap();
        match internal_pair.as_rule() {
            Rule::word => symbol_internal = SymbolInternal::Word(String::from(internal_pair.as_str())),
            Rule::varBind => symbol_internal = SymbolInternal::VarBind(String::from(internal_pair.as_str())),
            Rule::subtemplateCall => symbol_internal = SymbolInternal::SubtemplateCall(String::from(internal_pair.as_str())),
            Rule::template => symbol_internal = SymbolInternal::Template(Box::new(Self::parse_template_pair(internal_pair)?)),
            _ => return Err(ParseError::FailSymbol(String::from(pair_str))),
        }
        let optional = pair_str.ends_with("?");
        Ok(Symbol {
            symbol: symbol_internal,
            optional: optional,
        })
    }
}
