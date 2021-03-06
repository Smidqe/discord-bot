use crate::{db, serialization::string_or_struct, CONFIG};
use error_chain::error_chain;
use log::{debug, error, trace};
use maplit::hashmap;
use reqwest::{
    self, header,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serenity::{builder::CreateEmbed, http::Http, utils::Colour};
use std::{
    collections::HashSet,
    io,
    str::FromStr,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use void::Void;

error_chain! {
    links {
        Database(db::Error, db::ErrorKind);
    }

    foreign_links {
        Io(::std::io::Error);
        Discord(::serenity::Error);
        Reddit(::reqwest::Error);
        Json(::serde_json::Error);
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AccessTokenResponse {
    access_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RedditObject<T> {
    kind: String,
    data: T,
}

#[derive(Debug, Serialize, Deserialize)]
struct RedditListing<T> {
    after: Option<String>,
    before: Option<String>,
    children: Vec<RedditObject<T>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RedditMessageish {
    id: String,
    #[serde(default, deserialize_with = "string_or_struct")]
    replies: RedditObject<RedditListing<RedditMessageish>>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum NotificationClass {
    Modqueue,
    Modmail,
    ModmailReply,
}

impl NotificationClass {
    fn title(&self) -> &'static str {
        match *self {
            Self::Modqueue => "New stuff in the modqueue",
            Self::Modmail => "New modmail",
            Self::ModmailReply => "New reply to modmail",
        }
    }

    fn url<S>(&self, sub: S) -> String
    where
        S: AsRef<str>,
    {
        match *self {
            Self::Modqueue => format!("https://old.reddit.com/r/{}/about/modqueue/", sub.as_ref()),
            Self::Modmail | Self::ModmailReply => format!(
                "https://old.reddit.com/r/{}/about/message/inbox/",
                sub.as_ref()
            ),
        }
    }

    #[inline]
    fn colour(&self) -> Colour {
        match *self {
            Self::Modqueue => Colour::BLUE,
            Self::Modmail | Self::ModmailReply => Colour::RED,
        }
    }
}

impl<T> RedditListing<T> {
    #[inline]
    fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}

impl<T> Default for RedditListing<T> {
    #[inline]
    fn default() -> Self {
        Self {
            after: None,
            before: None,
            children: Vec::new(),
        }
    }
}

impl<T> Default for RedditObject<RedditListing<T>> {
    #[inline]
    fn default() -> Self {
        Self {
            kind: "Listing".to_owned(),
            data: RedditListing::default(),
        }
    }
}

impl<T> FromStr for RedditObject<RedditListing<T>> {
    type Err = Void;

    #[inline]
    fn from_str(_s: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self::default())
    }
}

fn make_client(
    auth_name: HeaderName,
    auth_value: HeaderValue,
) -> Result<reqwest::blocking::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static(concat!(
            "bot:fi.atte.",
            env!("CARGO_PKG_NAME"),
            ":v",
            env!("CARGO_PKG_VERSION"),
            " (by /u/AtteLynx)"
        )),
    );
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(auth_name, auth_value);

    Ok(reqwest::blocking::Client::builder()
        .referer(false)
        .default_headers(headers)
        .build()?)
}

fn make_login_client() -> Result<reqwest::blocking::Client> {
    make_client(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!(
            "Basic {}",
            ::base64::encode(&format!(
                "{}:{}",
                CONFIG.reddit.client_id, CONFIG.reddit.client_secret
            ))
        ))
        .unwrap(),
    )
}

// TODO: cache results
fn make_user_client() -> Result<reqwest::blocking::Client> {
    let resp = make_login_client()?
        .post("https://www.reddit.com/api/v1/access_token")
        .form(&hashmap! {
            "grant_type" => "password".to_owned(),
            "username" => CONFIG.reddit.username.to_string(),
            "password" => CONFIG.reddit.password.to_string(),
        })
        .send()?
        .error_for_status()?;

    let data: AccessTokenResponse = resp.json()?;
    Ok(make_client(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", data.access_token))
            .expect("invalid characters in access_token"),
    )?)
}

fn contains_unseen(database: &Connection, data: &RedditListing<RedditMessageish>) -> Result<bool> {
    let ids: Vec<String> = data
        .children
        .iter()
        .map(|obj| obj.data.id.clone())
        .collect();
    let has_unseen = db::reddit_contains_unseen(&database, ids.clone())?;
    db::reddit_seen(&database, ids)?;
    Ok(has_unseen)
}

fn check_sub(
    database: &Connection,
    client: &reqwest::blocking::Client,
    sub: &str,
) -> Result<HashSet<NotificationClass>> {
    debug!("Checking /r/{}", sub);
    let mut out = HashSet::new();

    {
        let data: RedditObject<RedditListing<RedditMessageish>> = client
            .get(&format!(
                "https://oauth.reddit.com/r/{}/about/modqueue",
                sub
            ))
            .send()?
            .error_for_status()?
            .json()?;

        if contains_unseen(&database, &data.data)? {
            out.insert(NotificationClass::Modqueue);
        }
    }

    {
        let data: RedditObject<RedditListing<RedditMessageish>> = client
            .get(&format!(
                "https://oauth.reddit.com/r/{}/about/message/inbox",
                sub
            ))
            .send()?
            .error_for_status()?
            .json()?;

        if contains_unseen(&database, &data.data)? {
            out.insert(NotificationClass::Modmail);
        }

        let mut replies = RedditListing::<RedditMessageish>::default();
        for msg in data.data.children {
            replies
                .children
                .extend(msg.data.replies.data.children.into_iter());
        }
        if !replies.is_empty() && contains_unseen(&database, &replies)? {
            out.insert(NotificationClass::ModmailReply);
        }
    }

    Ok(out)
}

fn apply_embed<'a>(
    e: &'a mut CreateEmbed,
    reddit_type: &NotificationClass,
    sub: &str,
    new: bool,
) -> &'a mut CreateEmbed {
    let e = e
        .colour(reddit_type.colour())
        .title(reddit_type.title())
        .url(reddit_type.url(sub))
        .author(|a| a.name(&format!("/r/{}", sub)));
    if new {
        e
    } else {
        e.description("(has been resolved)")
    }
}

fn main(http: &Arc<Http>) -> Result<()> {
    let database = db::connect()?;
    let client = make_user_client()?;
    for (sub, sub_config) in &CONFIG.subreddits {
        let sub = sub.as_ref();
        let reddit_types = check_sub(&database, &client, sub)?;
        for reddit_type in &reddit_types {
            for channel_id in &sub_config.notify_channels {
                channel_id.send_message(&http, |msg| {
                    msg.embed(|e| apply_embed(e, reddit_type, sub, true))
                })?;
            }
        }
        /*
        if !reddit_types.contains(&NotificationClass::Modqueue) {
            for channel_id in &sub_config.notify_channels {
                if let Some(mut msg) = channel_id
                    .messages(|req| req.limit(10))?
                    .into_iter()
                    .filter(|msg| msg.author.id == util::uid())
                    .last()
                {
                    msg.edit(|msg| {
                        msg.embed(|e| apply_embed(e, &NotificationClass::Modqueue, sub, false))
                    })?;
                }
            }
        }
        */
    }
    Ok(())
}

pub fn spawn(http: Arc<Http>) -> io::Result<thread::JoinHandle<()>> {
    if !CONFIG.reddit.enabled {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Reddit functionality is disabled in config",
        ));
    }

    trace!("Spawning Reddit thread...");

    let check_interval = Duration::from_secs(60 * CONFIG.reddit.check_interval);
    if check_interval.as_secs() < 60 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Reddit check interval is less than a minute; refusing",
        ));
    }

    thread::Builder::new()
        .name("reddit".to_owned())
        .spawn(move || {
            let mut start = Instant::now();
            loop {
                thread::sleep(Duration::from_secs(1).max(check_interval - start.elapsed()));
                if start.elapsed() >= check_interval {
                    start = Instant::now();
                    if let Err(err) = main(&http) {
                        error!("reddit error: {:?}", err);
                    }
                }
            }
        })
}
