use std::{cell::RefCell, collections::HashMap, error::Error, rc::Rc};
use futures::executor::block_on;

use cortex_lang::{interpreting::{env::Environment, interpreter::CortexInterpreter, module::Module, value::CortexValue}, parsing::{ast::{expression::{OptionalIdentifier, Parameter, PathIdent}, top_level::{Body, Function, Struct}, r#type::CortexType}, codegen::r#trait::SimpleCodeGen}};
use thiserror::Error;

use crate::templating::handler::TemplateHandler;

use super::spotify::spotify::Spotify;

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
    spotify: Option<Rc<RefCell<Spotify>>>,
}

impl CommandRunner {
    pub fn new() -> Self {
        CommandRunner {
            handler: TemplateHandler::new(),
            interpreter: CortexInterpreter::new(),
            spotify: None,
        }
    }

    pub fn init(&mut self, template_filepath: &str) -> Result<(), Box<dyn Error>> {
        self.handler.load_from_file(template_filepath)?;
        self.spotify = Some(Rc::new(RefCell::new(block_on(Spotify::init())?)));
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
            let fallback = self.handler.get_fallback()?;
            if let Some(func) = fallback {
                let _return_val = self.interpreter.call_function(&func, vec![CortexValue::String(String::from(input))])?;
            }
        }
        Ok(())
    }

    fn register_modules(&mut self) -> Result<(), Box<dyn Error>> {
        self.interpreter.register_module(&PathIdent::simple(String::from("Debug")), Self::build_debug_module()?)?;

        let spotify_module = block_on(Self::build_spotify_module(self.spotify.clone().unwrap()))?;
        self.interpreter.register_module(&PathIdent::simple(String::from("Spotify")), spotify_module)?;

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
    async fn build_spotify_module(spotify: Rc<RefCell<Spotify>>) -> Result<Module, Box<dyn Error>> {
        let mut mod_env = Environment::base();
        mod_env.add_struct(
            Struct::new(
                "Song", 
                vec![
                    ("id", CortexType::string(false)),
                    ("name", CortexType::string(false)),
                    ("artist", CortexType::string(false)),
                ]
            )
        )?;
        let sp1 = spotify.clone();
        mod_env.add_function(
            Function::new(
                OptionalIdentifier::Ident(String::from("search")),
                vec![Parameter::named("query", CortexType::string(false))],
                CortexType::new(PathIdent::new(vec!["Spotify", "Song"]), true),
                Body::Native(Box::new(move |env| {
                    let query = env.get_value("query")?;
                    if let CortexValue::String(string) = query {
                        let result = block_on(sp1.borrow_mut().get_song(string.clone()))?;
                        if let Some(song) = result {
                            Ok(CortexValue::Composite {
                                struct_name: PathIdent::new(vec!["Spotify", "Song"]),
                                field_values: HashMap::from([
                                    (String::from("id"), CortexValue::String(song.id)),
                                    (String::from("name"), CortexValue::String(song.name)),
                                    (String::from("artist"), CortexValue::String(song.artist)),
                                ]),
                            })
                        } else {
                            Ok(CortexValue::Null)
                        }
                    } else {
                        Ok(CortexValue::Null)
                    }
                }))
            )
        )?;
        let sp2 = spotify.clone();
        mod_env.add_function(
            Function::new(
                OptionalIdentifier::Ident(String::from("play")),
                vec![
                    Parameter::named("song_id", CortexType::string(false)),
                    Parameter::named("device_type", CortexType::number(false)),
                ],
                CortexType::void(false),
                Body::Native(Box::new(move |env| {
                    let song_id = env.get_value("song_id")?;
                    let device_type = env.get_value("device_type")?;
                    if let CortexValue::String(string) = song_id {
                        if let CortexValue::Number(typ) = device_type {
                            block_on(sp2.borrow_mut().play_song(string.clone(), *typ as u8))?;
                        }
                    }
                    Ok(CortexValue::Void)
                }))
            )
        )?;

        let module = Module::new(mod_env);
        Ok(module)
    }

}
