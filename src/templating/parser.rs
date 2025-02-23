use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
use thiserror::Error;

use super::template::{Clause, Symbol, SymbolInternal, Template};

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
        let pair = PestTemplateParser::parse(Rule::topTemplate, input);
        match pair {
            Ok(mut v) => Self::parse_template_pair(v.next().unwrap().into_inner().next().unwrap()),
            Err(_) => Err(ParseError::FailTemplate(String::from(input))),
        }
    }

    fn parse_template_pair(pair: Pair<Rule>) -> Result<Template, ParseError> {
        let mut clauses = Vec::new();
        let pairs = pair.into_inner();
        for p in pairs {
            if !matches!(p.as_rule(), Rule::EOI) {
                clauses.push(Self::parse_clause(p)?);
            }
        }
        Ok(Template { clauses: clauses })
    }

    fn parse_clause(pair: Pair<Rule>) -> Result<Clause, ParseError> {
        let mut symbols = Vec::new();
        for p in pair.into_inner() {
            symbols.push(Self::parse_symbol(p)?);
        }

        // Handle splitting text
        let mut true_symbols = Vec::new();
        for sym in symbols.into_iter() {
            let symbol_split = Self::split_words(sym.symbol);
            let optional = sym.optional;
            let len = symbol_split.len();
            let symbols_to_add: Vec<_> = symbol_split
                .into_iter()
                .enumerate()
                .map(|(i, internal)| {
                    if i == len - 1 {
                        Symbol::new(internal, optional)
                    } else {
                        Symbol::new(internal, false)
                    }
                })
                .collect();
            true_symbols.extend(symbols_to_add);
        }

        Ok(
            Clause {
                symbols: true_symbols,
            }
        )
    }

    fn parse_symbol(pair: Pair<Rule>) -> Result<Symbol, ParseError> {
        let pair_str = pair.as_str();
        let symbol_internal;
        let internal_pair = pair.into_inner().next().unwrap();
        match internal_pair.as_rule() {
            Rule::text => symbol_internal = SymbolInternal::Text(String::from(internal_pair.as_str())),
            Rule::varBind => symbol_internal = SymbolInternal::VarBind(String::from(internal_pair.into_inner().next().unwrap().as_str())),
            Rule::subtemplateCall => symbol_internal = SymbolInternal::SubtemplateCall(String::from(internal_pair.into_inner().next().unwrap().as_str())),
            Rule::template => symbol_internal = SymbolInternal::Template(Box::new(Self::parse_template_pair(internal_pair)?)),
            _ => return Err(ParseError::FailSymbol(String::from(pair_str))),
        }
        let optional = pair_str.ends_with("?");
        Ok(Symbol {
            symbol: symbol_internal,
            optional: optional,
        })
    }

    fn split_words(symbol: SymbolInternal) -> Vec<SymbolInternal> {
        if let SymbolInternal::Text(text) = symbol {
            let words_str: Vec<&str> = text.split_whitespace().collect();
            let words: Vec<SymbolInternal> = words_str.into_iter().map(|w| SymbolInternal::Text(String::from(w))).collect();
            words
        } else {
            vec![symbol]
        }
    }
}
