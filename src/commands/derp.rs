use super::super::CONFIG;
use rand::{self, Rng};
use reqwest::{self, Url};
use serenity::utils::Colour;

#[derive(Debug, Deserialize)]
pub struct Response {
    pub search: Vec<Search>,
}

#[derive(Debug, Deserialize)]
pub struct Search {
    pub id: u64,
    pub image: String,
    pub representations: SearchImages,
}

#[derive(Debug, Deserialize)]
pub struct SearchImages {
    pub thumb: String,
    pub medium: String,
}

command!(gib(_context, message, args) {
    let args = args.full();
    let tag = CONFIG
        .gib
        .aliases
        .iter()
        .find(|(_tag, aliases)| aliases.contains(args))
        .map_or(args, |(tag, _aliases)| tag.as_ref());

    let search = if CONFIG.gib.filters.sfw.tags.is_empty() {
        tag.replace(" ", "+")
    } else {
        format!("({}) AND ({})",
            CONFIG.gib.filters.sfw.tags.join(" AND "),
            tag.replace(" ", "+"))
    };

    let link = format!("https://derpibooru.org/search.json?min_score=100&sf=random%3A{}&perpage=1&filter_id={}&q={}",
        rand::thread_rng().gen::<u32>(),
        CONFIG.gib.filters.sfw.filter.to_string(),
        search
    );

    let mut curl = Url::parse(&link)?;
    let mut res = reqwest::get(curl)?;
    let json: Response = res.json()?;

    if json.search.is_empty() {
        let reply = rand::thread_rng()
                        .choose(&CONFIG.gib.not_found)
                        .map_or("", |reply| reply.as_ref());

        message.reply( &reply )?;
    } else {
        let reply = rand::thread_rng()
                        .choose(&CONFIG.gib.found)
                        .map_or("", |reply| reply.as_ref());

        let first = &json.search[0];
        message.channel_id.send_message(|msg| {
            msg.embed(|e|
                e.colour(Colour::gold())
                .description( &reply )
                .field("Link", format!("https://derpibooru.org/{}",first.id), false)
                .image(format!("http:{}",first.representations.medium))
            )
        })?;
    }
});
