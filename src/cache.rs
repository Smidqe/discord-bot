use serenity::{model::prelude::*, prelude::*};
use std::{
    collections::{HashMap, HashSet},
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
use toml;

error_chain! {
    foreign_links {
        Io(::std::io::Error);
        TomlRead(::toml::de::Error);
        TomlWrite(::toml::ser::Error);
    }
}

pub struct Cache {
    fh: RwLock<File>,
    content: RwLock<CacheContent>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheContent {
    pub reddit_seen: HashSet<String>,
    pub gib_seen: Vec<usize>,
    pub sticky_roles: HashMap<String, HashSet<RoleId>>,
}

impl Cache {
    fn new(fh: File, content: CacheContent) -> Self {
        Self {
            fh: RwLock::new(fh),
            content: RwLock::new(content),
        }
    }

    pub fn from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut fh = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        if fh.metadata()?.len() > 0 {
            let mut source: Vec<u8> = Vec::new();
            fh.read_to_end(&mut source)?;
            Ok(Self::new(fh, toml::from_slice(&source)?))
        } else {
            Ok(Self::new(fh, CacheContent::default()))
        }
    }

    pub fn with<F, R>(&self, fun: F) -> Result<R>
    where
        F: FnOnce(&mut CacheContent) -> R,
    {
        let (result, data) = {
            let mut content = self.content.write();
            let result = fun(&mut content);
            (result, toml::to_string(&*content)?)
        };

        let mut fh = self.fh.write();
        fh.seek(SeekFrom::Start(0))?;
        fh.set_len(0)?;
        fh.write_all(data.as_bytes())?;
        Ok(result)
    }
}
