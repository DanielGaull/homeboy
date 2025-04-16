use std::{error::Error, fs::File, io::{BufRead, BufReader}};

use cortex_lang::{interpreting::interpreter::CortexInterpreter, parsing::parser::CortexParser, preprocessing::ast::function::RFunction};
use thiserror::Error;

use super::{matcher::{Match, TemplateMatcher}, parser::TemplateParser, template::Template};

#[derive(Error, Debug)]
pub enum TemplateHandlerError {
    #[error("Illegal Line: {0}")]
    IllegalLine(String),
    #[error("Unexpected end of input (while {0})")]
    UnexpectedEof(&'static str),
}

pub struct TemplateHandler {
    matcher: TemplateMatcher,
    templates: Vec<TemplateEntry>,
    fallback: Option<RFunction>,
}

impl TemplateHandler {
    pub fn new() -> Self {
        TemplateHandler {
            matcher: TemplateMatcher::new(),
            templates: Vec::new(),
            fallback: None,
        }
    }

    pub fn find_function<'a>(&'a self, input: &str) -> Result<Option<MatchResult<'a>>, Box<dyn Error>> {
        for entry in &self.templates {
            let result = self.matcher.try_match(input, &entry.template)?;
            if let Some(mmatch) = result {
                let func = &entry.function;
                return Ok(Some(MatchResult {
                    function: func,
                    match_inst: mmatch,
                }));
            }
        }
        Ok(None)
    }

    pub fn get_fallback(&self) -> Result<Option<&RFunction>, Box<dyn Error>> {
        Ok(self.fallback.as_ref())
    }

    pub fn load_from_file(&mut self, filepath: &str, interpreter: &mut CortexInterpreter) -> Result<(), Box<dyn Error>> {
        let file = File::open(filepath)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines().peekable();
        while let Some(_) = lines.peek() {
            self.load_next_thing(&mut lines, interpreter)?;
        }
        Ok(())
    }

    fn load_next_thing(&mut self, iter: &mut dyn Iterator<Item = Result<String, std::io::Error>>, interpreter: &mut CortexInterpreter) -> Result<(), Box<dyn Error>> {
        loop {
            let mut line = iter.next().ok_or(TemplateHandlerError::UnexpectedEof("loading next element"))??;
            if line.trim().is_empty() {
                break;
            }

            if line.starts_with("% temp") {
                let template_line = iter.next().ok_or(TemplateHandlerError::UnexpectedEof("reading template header"))??;

                let mut function_lines = Vec::new();
                line = String::new();
                while !line.starts_with("% end") {
                    function_lines.push(line.clone());
                    line = iter.next().ok_or(TemplateHandlerError::UnexpectedEof("reading template function"))??;
                }
                let function_string = function_lines.into_iter().skip(1).collect::<Vec<_>>().join("\n");
                let template = TemplateParser::parse_template(&template_line)?;
                let function = CortexParser::parse_function(&function_string)?;
                let processed_function = interpreter.preprocess_function(function)?;
                let entry = TemplateEntry {
                    template: template,
                    function: processed_function,
                };
                self.templates.push(entry);
                break;
            } else if line.starts_with("% sub") {
                let name = iter.next().ok_or(TemplateHandlerError::UnexpectedEof("reading subtemplate header"))??;
                let mut subtemplate_lines = Vec::new();
                line = String::new();
                while !line.starts_with("% end") {
                    subtemplate_lines.push(line.clone());
                    line = iter.next().ok_or(TemplateHandlerError::UnexpectedEof("reading subtemplate body"))??;
                }
                let subtemplate_str = subtemplate_lines.join("");
                let subtemplate_template = TemplateParser::parse_template(&subtemplate_str)?;
                self.matcher.add_subtemplate(&name, subtemplate_template);
                break;
            } else if line.starts_with("% fallback") {
                let mut function_lines = Vec::new();
                line = String::new();
                while !line.starts_with("% end") {
                    function_lines.push(line.clone());
                    line = iter.next().ok_or(TemplateHandlerError::UnexpectedEof("reading fallback function"))??;
                }
                let function_string = function_lines.into_iter().skip(1).collect::<Vec<_>>().join("\n");
                let function = CortexParser::parse_function(&function_string)?;
                let processed_function = interpreter.preprocess_function(function)?;
                self.fallback = Some(processed_function);
                break;
            } else {
                return Err(Box::new(TemplateHandlerError::IllegalLine(line)));
            }
        }
        Ok(())
    }
}

struct TemplateEntry {
    template: Template,
    function: RFunction,
}

pub struct MatchResult<'a> {
    pub function: &'a RFunction,
    pub match_inst: Match,
}
