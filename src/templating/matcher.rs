use std::{collections::HashMap, error::Error};

use thiserror::Error;

use super::template::{SymbolInternal, Template};

#[derive(Error, Debug)]
pub enum TemplateError {
    #[error("Subtemplate \"{0}\" not found")]
    SubtemplateNotFound(String),
}

pub struct TemplateMatcher {
    subtemplate_definitions: HashMap<String, Template>,
}

impl TemplateMatcher {
    pub fn new() -> Self {
        TemplateMatcher {
            subtemplate_definitions: HashMap::new(),
        }
    }

    pub fn add_subtemplate(&mut self, name: &str, template: Template) {
        self.subtemplate_definitions.insert(String::from(name), template);
    }

    pub fn try_match(&self, input: String, template: Template) -> Option<Match> {

        None
    }

    pub fn convert_template_to_regex(&self, template: &Template) -> Result<String, TemplateError> {
        let joint_clauses: Vec<String> = template.clauses.iter().map(|c| {
            let joint_symbols: Vec<String> = c.symbols.iter().map(|sym| {
                let mut s = 
                    match &sym.symbol {
                        SymbolInternal::Text(t) => Ok(t.clone()),
                        SymbolInternal::SubtemplateCall(t) => {
                            let subtemplate = self.subtemplate_definitions.get(t);
                            if let Some(subt) = subtemplate {
                                Ok(format!("(?:{})", self.convert_template_to_regex(subt)?))
                            } else {
                                Err(TemplateError::SubtemplateNotFound(t.clone()))
                            }
                        },
                        SymbolInternal::VarBind(name) => {
                            Ok(format!("(?<{}>.*)", name.clone()))
                        },
                        SymbolInternal::Template(template) => {
                            let subtemplate_regex = self.convert_template_to_regex(&template)?;
                            Ok(format!("(?:{})", subtemplate_regex))
                        },
                    }?;
                if sym.optional {
                    s.push_str("?");
                }
                Ok(s)
            }).collect::<Result<Vec<String>, TemplateError>>()?;
            Ok(joint_symbols.join(" "))
        }).collect::<Result<Vec<String>, TemplateError>>()?;
        Ok(joint_clauses.join("|"))
    }
}

struct Match {
    variable_bindings: HashMap<String, String>,
}
impl Match {
    pub fn get_binding(&self, name: &String) -> Option<&String> {
        self.variable_bindings.get(name)
    }
}
