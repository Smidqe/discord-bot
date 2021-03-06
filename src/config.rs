use crate::substituting_string::SubstitutingString;
use error_chain::error_chain;
use serde::Deserialize;
use serenity::model::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::Path,
};
use toml;

error_chain! {
    foreign_links {
        Io(::std::io::Error);
        Toml(::toml::de::Error);
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub berrytube: BerrytubeConfig,
    pub discord: DiscordConfig,
    pub reddit: RedditConfig,
    pub subreddits: HashMap<SubstitutingString, SubredditConfig>,
    pub bulk: BulkConfig,
    pub gib: GibConfig,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub filename: SubstitutingString,
    pub log_queries: bool,
}

#[derive(Debug, Deserialize)]
pub struct BerrytubeConfig {
    pub enabled: bool,
    pub origin: SubstitutingString,
}

#[derive(Debug, Deserialize)]
pub struct DiscordConfig {
    pub command_prefix: SubstitutingString,
    pub deleted_msg_cache: u32,
    pub long_msg_threshold: usize,
    pub token: SubstitutingString,
    pub owners: HashSet<UserId>,
    pub log_channels: HashSet<ChannelId>,
    pub channel_blacklist: HashSet<ChannelId>,
    pub channel_whitelist: HashSet<ChannelId>,
    pub pin_channels: HashSet<ChannelId>,
    pub sticky_roles: HashSet<RoleId>,
}

#[derive(Debug, Deserialize)]
pub struct RedditConfig {
    pub enabled: bool,
    pub client_id: SubstitutingString,
    pub client_secret: SubstitutingString,
    pub username: SubstitutingString,
    pub password: SubstitutingString,
    pub check_interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct SubredditConfig {
    pub notify_channels: HashSet<ChannelId>,
}

#[derive(Debug, Deserialize)]
pub struct BulkConfig {
    pub insults: Vec<SubstitutingString>,
}

#[derive(Debug, Deserialize)]
pub struct GibConfig {
    pub filter: u32,
    pub history: u32,
    pub not_found: Vec<SubstitutingString>,
    pub aliases: HashMap<String, HashSet<String>>,
}

impl Config {
    pub fn from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut source: Vec<u8> = Vec::new();
        {
            let mut fh = File::open(path)?;
            fh.read_to_end(&mut source)?;
        }
        Ok(toml::from_slice(&source)?)
    }
}
