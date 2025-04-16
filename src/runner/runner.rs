use std::{cell::RefCell, env, error::Error, path::Path, rc::Rc};
use cortex_lang::{interpreting::{interpreter::CortexInterpreter, value::CortexValue}, parsing::ast::{expression::{OptionalIdentifier, Parameter, PathIdent}, top_level::{Body, PFunction, Struct}, r#type::CortexType}, preprocessing::module::Module};
use futures::executor::block_on;

use openweathermap::Volume;
use rdev::{listen, Event, EventType, Key, ListenError};
use thiserror::Error;

use crate::templating::handler::TemplateHandler;

use super::{location, memory::memory::{Memory, MemoryValue}, spotify::spotify::Spotify, voice::{deepgram::{DeepgramClient, OutputMode}, record::Recorder}};

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
    memory: Option<Rc<RefCell<Memory>>>,

    recorder: Option<Rc<RefCell<Recorder>>>,
    f8_down: bool,
    sp_button_pressed: bool, // Bluetooth headset requires button to be pressed once to record and again to stop
}

impl CommandRunner {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(
            CommandRunner {
                handler: TemplateHandler::new(),
                interpreter: CortexInterpreter::new()?,

                spotify: None,
                deepgram: None,
                memory: None,

                recorder: None,
                f8_down: false,
                sp_button_pressed: false,
            }
        )
    }

    pub fn init(&mut self, template_filepath: &str, output_mode: OutputMode) -> Result<(), Box<dyn Error>> {
        self.spotify = Some(Rc::new(RefCell::new(Spotify::new())));
        self.deepgram = Some(Rc::new(RefCell::new(DeepgramClient::init(output_mode)?)));
        self.memory = Some(Rc::new(RefCell::new(Memory::load(env::var("memory_path")?)?)));

        self.recorder = Some(Rc::new(RefCell::new(Recorder::new())));
        self.register_modules()?;
        self.handler.load_from_file(template_filepath, &mut self.interpreter)?;

        block_on(self.spotify.as_mut().unwrap().borrow_mut().init())?;

        Ok(())
    }

    pub fn get_input_devices(&self) -> Result<Vec<(usize, String)>, Box<dyn Error>> {
        self.recorder.as_ref().unwrap().borrow().get_input_devices()
    }
    pub fn set_input_device(&mut self, idx: usize) {
        self.recorder.as_mut().unwrap().borrow_mut().set_preferred_input_device(idx);
    }

    pub fn run_loop(mut self) -> Result<(), Box<dyn Error>> {
        println!("Ready");
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
                    self.on_record_start();
                }
            },
            EventType::KeyPress(Key::Unknown(179)) => {
                if self.sp_button_pressed {
                    self.on_record_stop();
                    self.sp_button_pressed = false;
                } else {
                    self.on_record_start();
                    self.sp_button_pressed = true;
                }
            },
            EventType::KeyRelease(Key::F8) => {
                self.f8_down = false;
                self.on_record_stop();
            },
            _ => {}
        }
    }
    fn on_record_start(&mut self) {
        println!("Recording started");
        let result = self
            .recorder
            .clone()
            .unwrap()
            .borrow_mut()
            .start_recording(Path::new("./recording.wav"));
        if let Err(error) = result {
            println!("{}", error);
            println!("Error when starting recording");
        }
    }
    fn on_record_stop(&mut self) {
        println!("Recording stopped");
        let result = self
            .recorder
            .clone()
            .unwrap()
            .borrow_mut()
            .stop_recording();
        if let Err(error) = result {
            println!("{}", error);
            println!("Error when stopping recording");
        }

        let result = self.handle_recording();
        if let Err(error) = result {
            println!("{}", error);
            println!("Error when handling recording");
        }
    }

    fn handle_recording(&mut self) -> Result<(), Box<dyn Error>> {
        let transcript = block_on(self.deepgram.clone().unwrap().borrow().transcribe(Path::new("./recording.wav")))?;
        println!("Transcript: {}", transcript);
        self.run(transcript.as_str())?;
        Ok(())
    }
    pub fn run(&mut self, input: &str) -> Result<(), Box<dyn Error>> {
        let sanitized_input: String = input.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();
        let result = self.handler.find_function(sanitized_input.as_str())?;
        if let Some(the_match) = result {
            let func = the_match.function;
            let inst = the_match.match_inst;
            let mut values = Vec::<CortexValue>::new();
            for i in 0..func.num_params() {
                let param = func.get_param(i).unwrap();
                let param_name = param;
                if let Some(binding) = inst.get_binding(param_name) {
                    values.push(CortexValue::String(binding.clone()));
                } else {
                    values.push(CortexValue::None);
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
        self.interpreter.register_module(&PathIdent::simple(String::from("Math")), Self::build_math_module()?)?;

        let spotify_module = Self::build_spotify_module(self.spotify.clone().unwrap())?;
        self.interpreter.register_module(&PathIdent::simple(String::from("Spotify")), spotify_module)?;

        let voice_module = Self::build_voice_module(self.deepgram.clone().unwrap())?;
        self.interpreter.register_module(&PathIdent::simple(String::from("Voice")), voice_module)?;

        let location_module = Self::build_location_module()?;
        self.interpreter.register_module(&PathIdent::simple(String::from("Location")), location_module)?;

        let weather_module = Self::build_weather_module()?;
        self.interpreter.register_module(&PathIdent::simple(String::from("Weather")), weather_module)?;

        let memory_module = Self::build_memory_module(self.memory.clone().unwrap())?;
        self.interpreter.register_module(&PathIdent::simple(String::from("Memory")), memory_module)?;

        Ok(())
    }
    fn build_debug_module() -> Result<Module, Box<dyn Error>> {
        let mut module = Module::new();
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("print")),
                vec![Parameter::named("text", CortexType::string(false))],
                CortexType::void(false), 
                Body::Native(Box::new(|env, _heap| {
                    let text = env.get_value("text")?;
                    if let CortexValue::String(string) = text {
                        println!("{}", string);
                    }
                    Ok(CortexValue::Void)
                })),
                vec![],
            )
        )?;
        Ok(module)
    }
    fn build_spotify_module(spotify: Rc<RefCell<Spotify>>) -> Result<Module, Box<dyn Error>> {
        let mut module = Module::new();
        module.add_struct(
            Struct::new(
                "Song", 
                vec![
                    ("id", CortexType::string(false)),
                    ("name", CortexType::string(false)),
                    ("artist", CortexType::string(false)),
                ],
                vec![],
            )
        )?;
        let sp1 = spotify.clone();
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("search")),
                vec![Parameter::named("query", CortexType::string(false))],
                CortexType::basic(PathIdent::new(vec!["Song"]), true, vec![]),
                Body::Native(Box::new(move |env, _heap| {
                    let query = env.get_value("query")?;
                    if let CortexValue::String(string) = query {
                        let result = block_on(sp1.borrow_mut().get_song(string.clone()))?;
                        if let Some(song) = result {
                            Ok(CortexValue::new_composite(vec![
                                ("id", CortexValue::String(song.id)),
                                ("name", CortexValue::String(song.name)),
                                ("artist", CortexValue::String(song.artist)),
                            ]))
                        } else {
                            Ok(CortexValue::None)
                        }
                    } else {
                        Ok(CortexValue::None)
                    }
                })),
                vec![]
            )
        )?;
        let sp2 = spotify.clone();
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("play")),
                vec![
                    Parameter::named("song_id", CortexType::string(false)),
                    Parameter::named("device_type", CortexType::number(false)),
                ],
                CortexType::void(false),
                Body::Native(Box::new(move |env, _heap| {
                    let song_id = env.get_value("song_id")?;
                    let device_type = env.get_value("device_type")?;
                    if let CortexValue::String(string) = song_id {
                        if let CortexValue::Number(typ) = device_type {
                            block_on(sp2.borrow().play_song(string.clone(), typ as u8))?;
                        }
                    }
                    Ok(CortexValue::Void)
                })),
                vec![]
            )
        )?;
        let sp3 = spotify.clone();
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("pause")),
                vec![],
                CortexType::void(false),
                Body::Native(Box::new(move |_env, _heap| {
                    block_on(sp3.borrow().pause())?;
                    Ok(CortexValue::Void)
                })),
                vec![]
            )
        )?;
        let sp4 = spotify.clone();
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("resume")),
                vec![],
                CortexType::void(false),
                Body::Native(Box::new(move |_env, _heap| {
                    block_on(sp4.borrow().resume())?;
                    Ok(CortexValue::Void)
                })),
                vec![],
            )
        )?;
        let sp5 = spotify.clone();
        module.add_function(PFunction::new(
            OptionalIdentifier::Ident(String::from("skip")),
            vec![],
            CortexType::void(false),
            Body::Native(Box::new(move |_env, _heap| {
                block_on(sp5.borrow().skip())?;
                Ok(CortexValue::Void)
            })),
            vec![]
        ))?;
        let sp6 = spotify.clone();
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("queue")),
                vec![
                    Parameter::named("song_id", CortexType::string(false)),
                    Parameter::named("device_type", CortexType::number(false)),
                ],
                CortexType::void(false),
                Body::Native(Box::new(move |env, _heap| {
                    let song_id = env.get_value("song_id")?;
                    let device_type = env.get_value("device_type")?;
                    if let CortexValue::String(string) = song_id {
                        if let CortexValue::Number(typ) = device_type {
                            block_on(sp6.borrow().queue_song(string.clone(), typ as u8))?;
                        }
                    }
                    Ok(CortexValue::Void)
                })),
                vec![]
            )
        )?;
        Ok(module)
    }

    fn build_voice_module(deepgram: Rc<RefCell<DeepgramClient>>) -> Result<Module, Box<dyn Error>> {
        let mut module = Module::new();
        let dg1 = deepgram.clone();
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("speak")),
                vec![Parameter::named("text", CortexType::string(false))],
                CortexType::void(false), 
                Body::Native(Box::new(move |env, _heap| {
                    let text = env.get_value("text")?;
                    if let CortexValue::String(string) = text {
                        block_on(dg1.borrow().speak(&string))?;
                    }
                    Ok(CortexValue::Void)
                })),
                vec![]
            ),
        )?;
        Ok(module)
    }
    fn build_weather_module() -> Result<Module, Box<dyn Error>> {
        let mut module = Module::new();
        module.add_struct(Struct::new(
            "Volume",
            vec![
                ("lastHour", CortexType::number(true)),
                ("last3Hours", CortexType::number(true)),
            ],
            vec![]
        ))?;
        module.add_struct(Struct::new(
            "Report", 
            vec![
                ("temp", CortexType::number(false)),
                ("windSpeed", CortexType::number(false)),
                ("windDirection", CortexType::number(false)),
                ("windGust", CortexType::number(true)),
                ("feelsLike", CortexType::number(false)),
                ("humidity", CortexType::number(false)),
                ("rain", CortexType::basic(PathIdent::new(vec!["Volume"]), true, vec![])),
                ("snow", CortexType::basic(PathIdent::new(vec!["Volume"]), true, vec![])),
            ],
            vec![]
        ))?;
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("get")),
                vec![
                    Parameter::named("latitude", CortexType::number(false)),
                    Parameter::named("longitude", CortexType::number(false)),
                ],
                CortexType::basic(PathIdent::new(vec!["Report"]), true, vec![]), 
                Body::Native(Box::new(move |env, _heap| {
                    let lat = env.get_value("latitude")?;
                    let long = env.get_value("longitude")?;
                    let latitude = unwrap_enum!(lat, CortexValue::Number(v) => v);
                    let longitude = unwrap_enum!(long, CortexValue::Number(v) => v);
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
                                    Some(v) => CortexValue::new_composite(vec![
                                        ("lastHour", match v.h1 {
                                            Some(h) => CortexValue::Number(h),
                                            None => CortexValue::None,
                                        }),
                                        ("last3Hour", match v.h3 {
                                            Some(h) => CortexValue::Number(h),
                                            None => CortexValue::None,
                                        }),
                                    ]),
                                    None => CortexValue::None,
                                }
                            }

                            let rain = volume_to_struct(&current.rain);
                            let snow = volume_to_struct(&current.snow);
                            CortexValue::new_composite(vec![
                                ("temp", CortexValue::Number(current.main.temp)),
                                ("windSpeed", CortexValue::Number(current.wind.speed)),
                                ("windDirection", CortexValue::Number(current.wind.deg)),
                                ("windGust", match current.wind.gust {
                                    Some(g) => CortexValue::Number(g),
                                    None => CortexValue::None,
                                }),
                                ("feelsLike", CortexValue::Number(current.main.feels_like)),
                                ("humidity", CortexValue::Number(current.main.humidity)),
                                ("rain", rain),
                                ("snow", snow),
                            ])
                        },
                        Err(e) => {
                            println!("Could not fetch weather because: {}", e);
                            CortexValue::None
                        },
                    };
                    Ok(val)
                })),
                vec![]
            ),
        )?;
        Ok(module)
    }

    fn build_location_module() -> Result<Module, Box<dyn Error>> {
        let mut module = Module::new();
        module.add_struct(Struct::new(
            "Location", 
            vec![
                ("long", CortexType::number(false)),
                ("lat", CortexType::number(false)),
                ("name", CortexType::string(false)),
            ],
            vec![]
        ))?;
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("get")),
                vec![],
                CortexType::basic(PathIdent::new(vec!["Location"]), false, vec![]),
                Body::Native(Box::new(move |_env, _heap| {
                    let loc = block_on(location::get_loc())?;
                    Ok(CortexValue::new_composite(vec![
                        ("long", CortexValue::Number(loc.long)),
                        ("lat", CortexValue::Number(loc.lat)),
                        ("name", CortexValue::String(loc.city)),
                    ]))
                })),
                vec![]
            )
        )?;
        Ok(module)
    }

    fn build_math_module() -> Result<Module, Box<dyn Error>> {
        let mut module = Module::new();
        module.add_function(PFunction::new(
            OptionalIdentifier::Ident(String::from("floor")), 
            vec![Parameter::named("numberInput", CortexType::number(false))],
            CortexType::number(false), 
            Body::Native(Box::new(|env, _heap| {
                let num = env.get_value("numberInput")?;
                let val = match num {
                    CortexValue::Number(n) => CortexValue::Number(n.floor()),
                    _ => num.clone(),
                };
                Ok(val)
            })),
            vec![]
        ))?;
        Ok(module)
    }

    fn build_memory_module(memory: Rc<RefCell<Memory>>) -> Result<Module, Box<dyn Error>> {
        let mut module = Module::new();
        let m1 = memory.clone();
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("get")),
                vec![Parameter::named("key", CortexType::string(false))],
                CortexType::string(true),
                Body::Native(Box::new(move |env, _heap| {
                    let key = env.get_value("key")?;
                    let key = unwrap_enum!(key, CortexValue::String(v) => v);
                    let memory = m1.borrow().get(&key);
                    if let Some(m) = memory {
                        if let MemoryValue::Single(s) = m {
                            Ok(CortexValue::String(s))
                        } else {
                            Ok(CortexValue::None)
                        }
                    } else {
                        Ok(CortexValue::None)
                    }
                })),
                vec![],
            )
        )?;
        let m2 = memory.clone();
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("getl")),
                vec![Parameter::named("key", CortexType::string(false))],
                CortexType::reference(CortexType::list(CortexType::string(false), true), true),
                Body::Native(Box::new(move |env, heap| {
                    let key = env.get_value("key")?;
                    let key = unwrap_enum!(key, CortexValue::String(v) => v);
                    let memory = m2.borrow().get(&key);
                    if let Some(m) = memory {
                        if let MemoryValue::List(l) = m {
                            let list = CortexValue::List(l.into_iter().map(|s| CortexValue::String(s)).collect());
                            let addr = heap.allocate(list);
                            Ok(CortexValue::Reference(addr))
                        } else {
                            Ok(CortexValue::None)
                        }
                    } else {
                        Ok(CortexValue::None)
                    }
                })),
                vec![],
            )
        )?;
        let m3 = memory.clone();
        module.add_function(
            PFunction::new(
                OptionalIdentifier::Ident(String::from("set")),
                vec![
                    Parameter::named("key", CortexType::string(false)),
                    Parameter::named("value", CortexType::simple("T", false))
                ],
                CortexType::void(false),
                Body::Native(Box::new(move |env, heap| {
                    let key = env.get_value("key")?;
                    let key = unwrap_enum!(key, CortexValue::String(v) => v);
                    let value = env.get_value("value")?;
                    if let CortexValue::Reference(addr) = value {
                        let ref_val = heap.get(addr);
                        if let CortexValue::List(ref items) = *ref_val.borrow() {
                            let value = items.iter().map(|v| to_string(v)).collect::<Vec<_>>();
                            m3.borrow_mut().set(key, MemoryValue::List(value));
                        } else {
                            m3.borrow_mut().set(key, MemoryValue::Single(to_string(&*ref_val.borrow())));
                        };
                    } else {
                        m3.borrow_mut().set(key, MemoryValue::Single(to_string(&value)));
                    }
                    m3.borrow().save()?;
                    
                    Ok(CortexValue::Void)
                })),
                vec![String::from("T")],
            )
        )?;

        Ok(module)
    }
}

fn to_string(value: &CortexValue) -> String {
    match value {
        CortexValue::Number(v) => v.to_string(),
        CortexValue::Boolean(v) => v.to_string(),
        CortexValue::String(v) => v.clone(),
        CortexValue::Char(v) => (*v as char).to_string(),
        CortexValue::Void => String::from("<void>"),
        CortexValue::None => String::from("<none>"),
        CortexValue::Composite { field_values: _ } => String::from("<composite>"),
        CortexValue::Reference(_) => String::from("<ref>"),
        CortexValue::List(_) => String::from("<list>"),
    }
}
