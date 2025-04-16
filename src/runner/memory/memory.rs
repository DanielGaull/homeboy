use std::{collections::HashMap, fs::{File, OpenOptions}, io::{self, BufRead, Write}, path::{Path, PathBuf}};

#[derive(Clone)]
pub enum MemoryValue {
    Single(String),
    List(Vec<String>),
}
impl ToString for MemoryValue {
    fn to_string(&self) -> String {
        match self {
            MemoryValue::Single(s) => s.clone(),
            MemoryValue::List(vs) => format!("[{}]", vs.join(", ")),
        }
    }
}

pub struct Memory {
    memories: HashMap<String, MemoryValue>,
    path: PathBuf,
}

impl Memory {
    pub fn load<P: AsRef<Path>>(filename: P) -> io::Result<Self> {
        let file = File::open(&filename)?;
        let reader = io::BufReader::new(file);

        let mut memories = HashMap::new();
    
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
    
            // Skip empty lines or lines that don't contain '='
            if trimmed.is_empty() || !trimmed.contains('=') || trimmed.starts_with("//") {
                continue;
            }
    
            let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
            let key = parts[0].trim().to_string();
            let value_raw = parts[1].trim();
    
            let value = if value_raw.starts_with('[') && value_raw.ends_with(']') {
                let inner = &value_raw[1..value_raw.len() - 1]; // remove brackets
                let list = inner
                    .split(',')
                    .map(|item| item.trim().to_string())
                    .collect();
                MemoryValue::List(list)
            } else {
                MemoryValue::Single(value_raw.to_string())
            };
    
            memories.insert(key, value);
        }
    
        Ok(Memory {
            memories,
            path: filename.as_ref().to_path_buf(),
        })
    }
    pub fn save(&self) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)?;

        for (key, value) in &self.memories {
            writeln!(file, "{}={}", key, value.to_string())?;
        }

        Ok(())
    }

    pub fn get(&self, key: &String) -> Option<MemoryValue> {
        self.memories.get(key).cloned()
    }
    pub fn set(&mut self, key: String, value: MemoryValue) {
        self.memories.insert(key, value);
    }
}
