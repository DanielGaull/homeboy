use std::error::Error;

use cortex_lang::{interpreting::{env::Environment, interpreter::CortexInterpreter, module::Module, value::CortexValue}, parsing::{ast::{expression::{OptionalIdentifier, Parameter, PathIdent}, top_level::{Body, Function}, r#type::CortexType}, codegen::r#trait::SimpleCodeGen}};
use thiserror::Error;

use crate::templating::handler::TemplateHandler;

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("Binding for required parameter '{0}' not found")]
    BindingNotFound(String),
    #[error("Invalid parameter type '{0}'. Parameters must be string or string?")]
    InvalidParameterType(String),
}

pub struct CommandRunner {
    handler: TemplateHandler,
    interpreter: CortexInterpreter,
}

impl CommandRunner {
    pub fn new() -> Self {
        CommandRunner {
            handler: TemplateHandler::new(),
            interpreter: CortexInterpreter::new(),
        }
    }

    pub fn init(&mut self, template_filepath: &str) -> Result<(), Box<dyn Error>> {
        self.handler.load_from_file(template_filepath)?;
        self.register_modules()?;
        Ok(())
    }

    pub fn run(&mut self, input: &str) -> Result<(), Box<dyn Error>> {
        let result = self.handler.find_function(input)?;
        if let Some(the_match) = result {
            let func = the_match.function;
            let inst = the_match.match_inst;
            let mut values = Vec::<CortexValue>::new();
            for i in 0..func.num_params() {
                let param = func.get_param(i).unwrap();
                let param_name = param.name();
                let param_type = param.param_type();
                if !param_type.is_subtype_of(&CortexType::string(true)) {
                    return Err(Box::new(RunnerError::InvalidParameterType(param_type.codegen(0))));
                }
                if let Some(binding) = inst.get_binding(param_name) {
                    values.push(CortexValue::String(binding.clone()));
                } else {
                    if !param_type.nullable() {
                        return Err(Box::new(RunnerError::BindingNotFound(param_name.clone())));
                    }
                    values.push(CortexValue::Null);
                }
            }
            let _return_val = self.interpreter.call_function(func, values)?;
        } else {
            // TODO: fallback (match wasn't found)
        }
        Ok(())
    }

    fn register_modules(&mut self) -> Result<(), Box<dyn Error>> {
        self.interpreter.register_module(&PathIdent::simple(String::from("Debug")), Self::build_debug_module()?)?;
        Ok(())
    }
    fn build_debug_module() -> Result<Module, Box<dyn Error>> {
        let mut mod_env = Environment::base();
        mod_env.add_function(
            Function::new(
                OptionalIdentifier::Ident(String::from("print")),
                vec![Parameter::named("text", CortexType::string(false))],
                CortexType::void(false), 
                Body::Native(Box::new(|env| {
                    let text = env.get_value("text")?;
                    if let CortexValue::String(string) = text {
                        println!("{}", string);
                    }
                    Ok(CortexValue::Void)
                }))
            )
        )?;
        let module = Module::new(mod_env);
        Ok(module)
    }

}
