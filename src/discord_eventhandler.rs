use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::utils::Colour;
use serenity::CACHE;

use super::CONFIG;

lazy_static! {
    pub static ref MESSAGE_CACHE: RwLock<Vec<Message>> = RwLock::new(Vec::new());
}

fn get_log_channels(guild_id: GuildId) -> Vec<ChannelId> {
    CONFIG
        .discord
        .log_channels
        .iter()
        .filter_map(|id| {
            if CACHE
                .read()
                .channels
                .get(id)
                .map_or(false, |chan| chan.read().guild_id == guild_id)
            {
                Some(*id)
            } else {
                None
            }
        })
        .collect()
}

pub struct Handler;

impl EventHandler for Handler {
    fn ready(&self, context: Context, _: Ready) {
        let wanted_name: &str = CONFIG.discord.username.as_ref();
        if CACHE.read().user.name != wanted_name {
            if let Err(err) = context.edit_profile(|p| p.username(wanted_name)) {
                warn!("Error settings username: {:?}", err);
            }
        }
        context.set_game(Game::listening(&format!(
            "{}help",
            CONFIG.discord.command_prefix.as_ref() as &str
        )));
    }

    fn message(&self, _context: Context, message: Message) {
        let mut cache = MESSAGE_CACHE.write();
        cache.insert(0, message);
        cache.truncate(CONFIG.discord.deleted_msg_cache);
    }

    fn message_update(&self, _context: Context, update: MessageUpdateEvent) {
        if let Some(message) = MESSAGE_CACHE
            .write()
            .iter_mut()
            .find(|msg| msg.id == update.id)
        {
            if let Some(content) = update.content {
                message.content = content;
            }
        }
    }

    fn message_delete(&self, _context: Context, channel_id: ChannelId, message_id: MessageId) {
        if CONFIG.discord.log_channels.contains(&channel_id) {
            return;
        }

        if let Ok(Channel::Guild(channel)) = channel_id.get() {
            let channel = channel.read();
            if let Some(message) = MESSAGE_CACHE.read().iter().find(|msg| msg.id == message_id) {
                for log_channel in get_log_channels(channel.guild_id) {
                    if let Err(err) = log_channel.send_message(|msg| {
                        msg.embed(|e| {
                            e.colour(Colour::red())
                                .title(format!("Message deleted in #{}", channel.name()))
                                .description(message.content_safe())
                                .author(|a| {
                                    a.name(&message.author.tag())
                                        .icon_url(&message.author.face())
                                })
                                .timestamp(&message.timestamp)
                        })
                    }) {
                        warn!("Unable to add message deletion to log channel: {:?}", err);
                    }
                }
            } else {
                info!("Unable to find deleted message in cache!");
            }
        } else {
            warn!("Unable to get channel for deleted message!");
        }
    }

    fn guild_member_addition(&self, _context: Context, guild_id: GuildId, member: Member) {
        for log_channel in get_log_channels(guild_id) {
            if let Err(err) = log_channel.send_message(|msg| {
                let user = member.user.read();
                msg.embed(|e| {
                    e.colour(Colour::fooyoo())
                        .title("User joined")
                        .author(|a| a.name(&user.tag()).icon_url(&user.face()))
                })
            }) {
                warn!("Unable to add member join to log channel: {:?}", err);
            }
        }
    }

    fn guild_member_removal(
        &self,
        _context: Context,
        guild_id: GuildId,
        user: User,
        _member: Option<Member>,
    ) {
        for log_channel in get_log_channels(guild_id) {
            if let Err(err) = log_channel.send_message(|msg| {
                msg.embed(|e| {
                    e.colour(Colour::red())
                        .title("User left")
                        .author(|a| a.name(&user.tag()).icon_url(&user.face()))
                })
            }) {
                warn!("Unable to add member leave to log channel: {:?}", err);
            }
        }
    }

    fn guild_member_update(
        &self,
        _context: Context,
        old_member: Option<Member>,
        new_member: Member,
    ) {
        if let Some(old_member) = old_member {
            if old_member.nick == new_member.nick {
                return;
            }

            let old_user = old_member.user.read();
            let new_user = new_member.user.read();

            let old_nick = old_member.nick.unwrap_or_else(|| old_user.name.clone());
            let new_nick = new_member.nick.unwrap_or_else(|| new_user.name.clone());

            for log_channel in get_log_channels(old_member.guild_id) {
                if let Err(err) = log_channel.send_message(|msg| {
                    msg.embed(|e| {
                        e.colour(Colour::red())
                            .title("User changed their nick")
                            .description(format!("{} \u{2192} {}", old_nick, new_nick))
                            .author(|a| a.name(&new_user.tag()).icon_url(&new_user.face()))
                    })
                }) {
                    warn!("Unable to add nick change to log channel: {:?}", err);
                }
            }
        }
    }
}