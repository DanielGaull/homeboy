use std::{cell::RefCell, collections::HashMap, env, error::Error, path::Path, rc::Rc};
use futures::executor::block_on;

use cortex_lang::{interpreting::{env::Environment, interpreter::CortexInterpreter, module::Module, value::CortexValue}, parsing::{ast::{expression::{OptionalIdentifier, Parameter, PathIdent}, top_level::{Body, Function, Struct}, r#type::CortexType}, codegen::r#trait::SimpleCodeGen}};
use openweathermap::Volume;
use rdev::{listen, Event, EventType, Key, ListenError};
use thiserror::Error;

use crate::templating::handler::TemplateHandler;

use super::{location, spotify::spotify::Spotify, voice::{deepgram::DeepgramClient, record::Recorder}};

macro_rules! unwrap_enum {
    ($e:expr, $p:pat => $v:expr) => {
        match $e {
            $p => $v,
            _ => panic!("Unexpected variant"),
        }
    };
}

#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("Binding for required parameter '{0}' not found")]
    BindingNotFound(String),
    #[error("Invalid parameter type '{0}'. Parameters must be string or string?")]
    InvalidParameterType(String),
    #[error("There was a listen error")]
    ListenError(ListenError),
}

pub struct CommandRunner {
    handler: TemplateHandler,
    interpreter: CortexInterpreter,
    spotify: Option<Rc<RefCell<Spotify>>>,
    deepgram: Option<Rc<RefCell<DeepgramClient>>>,
    recorder: Option<Rc<RefCell<Recorder>>>,
    f8_down: bool,
}

impl CommandRunner {
    pub fn new() -> Self {
        CommandRunner {
            handler: TemplateHandler::new(),
            interpreter: CortexInterpreter::new(),
            spotify: None,
            deepgram: None,
            recorder: None,
            f8_down: false,
        }
    }

    pub fn init(&mut self, template_filepath: &str) -> Result<(), Box<dyn Error>> {
        self.handler.load_from_file(template_filepath)?;
        self.spotify = Some(Rc::new(RefCell::new(block_on(Spotify::init())?)));
        self.deepgram = Some(Rc::new(RefCell::new(DeepgramClient::init()?)));
        self.recorder = Some(Rc::new(RefCell::new(Recorder::new())));
        self.register_modules()?;
        Ok(())
    }

    pub fn get_input_devices(&self) -> Result<Vec<(usize, String)>, Box<dyn Error>> {
        self.recorder.as_ref().unwrap().borrow().get_input_devices()
    }
    pub fn set_input_device(&mut self, idx: usize) {
        self.recorder.as_mut().unwrap().borrow_mut().set_preferred_input_device(idx);
    }

    pub fn run_loop(mut self) -> Result<(), Box<dyn Error>> {
        println!("Listening");
        if let Err(error) = listen(move |event| self.handle_key_event(event)) {
            return Err(Box::new(RunnerError::ListenError(error)));
        }
        Ok(())
    }
    fn handle_key_event(&mut self, event: Event) {
        match event.event_type {
            EventType::KeyPress(Key::F8) => {
                if !self.f8_down {
                    self.f8_down = true;
                    let result = self
                        .recorder
                        .clone()
                        .unwrap()
                        .borrow_mut()
                        .start_recording(Path::new("./recording.wav"));
                    if let Err(error) = result {
                        println!("{}", error);
                        panic!("Error when starting recording");
                    }
                }
            }
            EventType::KeyRelease(Key::F8) => {
                self.f8_down = false;
                let result = self
                    .recorder
                    .clone()
                    .unwrap()
                    .borrow_mut()
                    .stop_recording();
                if let Err(error) = result {
                    println!("{}", error);
                    panic!("Error when stopping recording");
                }

                let result = self.handle_recording();
                if let Err(error) = result {
                    println!("{}", error);
                    panic!("Error when handling recording");
                }
            }
            _ => {}
        }
    }

    fn handle_recording(&mut self) -> Result<(), Box<dyn Error>> {
        let transcript = block_on(self.deepgram.clone().unwrap().borrow().transcribe(Path::new("./recording.wav")))?;
        println!("Transcript: {}", transcript);
        let command: String = transcript
            .as_str()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect();
        self.run(command.as_str())?;
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
        self.interpreter.register_module(&PathIdent::simple(String::from("Util")), Self::build_util_module()?)?;

        let spotify_module = block_on(Self::build_spotify_module(self.spotify.clone().unwrap()))?;
        self.interpreter.register_module(&PathIdent::simple(String::from("Spotify")), spotify_module)?;

        let voice_module = block_on(Self::build_voice_module(self.deepgram.clone().unwrap()))?;
        self.interpreter.register_module(&PathIdent::simple(String::from("Voice")), voice_module)?;

        let location_module = Self::build_location_module()?;
        self.interpreter.register_module(&PathIdent::simple(String::from("Location")), location_module)?;

        let weather_module = Self::build_weather_module()?;
        self.interpreter.register_module(&PathIdent::simple(String::from("Weather")), weather_module)?;

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
        let sp3 = spotify.clone();
        mod_env.add_function(
            Function::new(
                OptionalIdentifier::Ident(String::from("pause")),
                vec![],
                CortexType::void(false),
                Body::Native(Box::new(move |_env| {
                    block_on(sp3.borrow_mut().pause())?;
                    Ok(CortexValue::Void)
                }))
            )
        )?;
        let sp4 = spotify.clone();
        mod_env.add_function(
            Function::new(
                OptionalIdentifier::Ident(String::from("resume")),
                vec![],
                CortexType::void(false),
                Body::Native(Box::new(move |_env| {
                    block_on(sp4.borrow_mut().resume())?;
                    Ok(CortexValue::Void)
                }))
            )
        )?;

        let module = Module::new(mod_env);
        Ok(module)
    }

    async fn build_voice_module(deepgram: Rc<RefCell<DeepgramClient>>) -> Result<Module, Box<dyn Error>> {
        let mut mod_env = Environment::base();
        let dg1 = deepgram.clone();
        mod_env.add_function(
            Function::new(
                OptionalIdentifier::Ident(String::from("speak")),
                vec![Parameter::named("text", CortexType::string(false))],
                CortexType::void(false), 
                Body::Native(Box::new(move |env| {
                    let text = env.get_value("text")?;
                    if let CortexValue::String(string) = text {
                        block_on(dg1.borrow().speak(string))?;
                    }
                    Ok(CortexValue::Void)
                }))
            )
        )?;
        let module = Module::new(mod_env);
        Ok(module)
    }
    fn build_weather_module() -> Result<Module, Box<dyn Error>> {
        let mut mod_env = Environment::base();
        mod_env.add_struct(Struct::new(
            "Volume",
            vec![
                ("lastHour", CortexType::number(true)),
                ("last3Hours", CortexType::number(true)),
            ],
        ))?;
        mod_env.add_struct(Struct::new(
            "Report", 
            vec![
                ("temp", CortexType::number(false)),
                ("windSpeed", CortexType::number(false)),
                ("windDirection", CortexType::number(false)),
                ("windGust", CortexType::number(true)),
                ("feelsLike", CortexType::number(false)),
                ("humidity", CortexType::number(false)),
                ("rain", CortexType::new(PathIdent::new(vec!["Weather", "Volume"]), true)),
                ("snow", CortexType::new(PathIdent::new(vec!["Weather", "Volume"]), true)),
            ],
        ))?;
        mod_env.add_function(
            Function::new(
                OptionalIdentifier::Ident(String::from("get")),
                vec![
                    Parameter::named("latitude", CortexType::number(false)),
                    Parameter::named("longitude", CortexType::number(false)),
                ],
                CortexType::new(PathIdent::new(vec!["Weather", "Report"]), true), 
                Body::Native(Box::new(move |env| {
                    let lat = env.get_value("latitude")?;
                    let long = env.get_value("longitude")?;
                    let latitude = unwrap_enum!(lat, CortexValue::Number(v) => *v);
                    let longitude = unwrap_enum!(long, CortexValue::Number(v) => *v);
                    let weather = &openweathermap::blocking::weather(
                        format!("{},{}", latitude, longitude).as_str(), 
                        "imperial", 
                        "en", 
                        env::var("open_weather_api_key")?.as_str()
                    );
                    let val = match weather {
                        Ok(current) => {                            
                            fn volume_to_struct(volume: &Option<Volume>) -> CortexValue {
                                match volume {
                                    Some(v) => CortexValue::Composite { 
                                        struct_name: PathIdent::new(vec!["Weather", "Volume"]), 
                                        field_values: HashMap::from([
                                            (String::from("lastHour"), match v.h1 {
                                                Some(h) => CortexValue::Number(h),
                                                None => CortexValue::Null,
                                            }),
                                            (String::from("last3Hour"), match v.h3 {
                                                Some(h) => CortexValue::Number(h),
                                                None => CortexValue::Null,
                                            }),
                                        ]),
                                    },
                                    None => CortexValue::Null,
                                }
                            }

                            let rain = volume_to_struct(&current.rain);
                            let snow = volume_to_struct(&current.snow);
                            CortexValue::Composite { 
                                struct_name: PathIdent::new(vec!["Weather", "Report"]),
                                field_values: HashMap::from([
                                    (String::from("temp"), CortexValue::Number(current.main.temp)),
                                    (String::from("windSpeed"), CortexValue::Number(current.wind.speed)),
                                    (String::from("windDirection"), CortexValue::Number(current.wind.deg)),
                                    (String::from("windGust"), match current.wind.gust {
                                        Some(g) => CortexValue::Number(g),
                                        None => CortexValue::Null,
                                    }),
                                    (String::from("feelsLike"), CortexValue::Number(current.main.feels_like)),
                                    (String::from("humidity"), CortexValue::Number(current.main.humidity)),
                                    (String::from("rain"), rain),
                                    (String::from("snow"), snow),
                                ]),
                            }
                        },
                        Err(e) => {
                            println!("Could not fetch weather because: {}", e);
                            CortexValue::Null
                        },
                    };
                    Ok(val)
                }))
            )
        )?;
        let module = Module::new(mod_env);
        Ok(module)
    }

    fn build_location_module() -> Result<Module, Box<dyn Error>> {
        let mut mod_env = Environment::base();
        mod_env.add_struct(Struct::new(
            "Location", 
            vec![
                ("long", CortexType::number(false)),
                ("lat", CortexType::number(false)),
                ("name", CortexType::string(false)),
            ]
        ))?;
        mod_env.add_function(
            Function::new(
                OptionalIdentifier::Ident(String::from("get")),
                vec![],
                CortexType::new(PathIdent::new(vec!["Location", "Location"]), false),
                Body::Native(Box::new(move |_env| {
                    let loc = block_on(location::get_loc())?;
                    Ok(CortexValue::Composite { 
                        struct_name: PathIdent::new(vec!["Location", "Location"]), 
                        field_values: HashMap::from([
                            (String::from("long"), CortexValue::Number(loc.long)),
                            (String::from("lat"), CortexValue::Number(loc.lat)),
                            (String::from("name"), CortexValue::String(loc.city)),
                        ]),
                    })
                }))
            )
        )?;
        let module = Module::new(mod_env);
        Ok(module)
    }

    fn build_util_module() -> Result<Module, Box<dyn Error>> {
        let mut mod_env = Environment::base();
        mod_env.add_function(Function::new(
            OptionalIdentifier::Ident(String::from("n2s")), 
            vec![Parameter::named("numberInput", CortexType::number(false))],
            CortexType::string(false), 
            Body::Native(Box::new(|env| {
                let num = env.get_value("numberInput")?;
                let val = match num {
                    CortexValue::Number(n) => CortexValue::String(format!("{}", n)),
                    _ => CortexValue::String(String::from(""))
                };
                Ok(val)
            })),
        ))?;
        let module = Module::new(mod_env);
        Ok(module)
    }

}
