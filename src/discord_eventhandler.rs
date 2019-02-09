use crate::{util, CONFIG};
use lazy_static::lazy_static;
use log::{info, warn};
use rand::{self, seq::SliceRandom};
use serenity::{model::prelude::*, prelude::*, utils::Colour, CACHE};
use std::collections::HashSet;

lazy_static! {
    pub static ref MESSAGE_CACHE: RwLock<Vec<Message>> = RwLock::new(Vec::new());
}

pub fn get_log_channels(guild_id: GuildId) -> Vec<ChannelId> {
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
        let current_name = CACHE.read().user.name.clone();
        if current_name != wanted_name {
            if let Err(err) = CACHE.write().user.edit(|p| p.username(wanted_name)) {
                warn!("Error settings username: {:?}", err);
            }
        }
        context.set_game(Game::listening(&format!(
            "{}help",
            CONFIG.discord.command_prefix.as_ref() as &str
        )));
    }

    fn message(&self, _context: Context, message: Message) {
        if util::can_respond_to(&message)
            && message.mentions.iter().any(|user| user.id == util::uid())
        {
            if let Some(insult) = CONFIG.bulk.insults.choose(&mut rand::thread_rng()) {
                message.reply(insult.as_ref()).ok();
            }
        }

        if message.content.to_lowercase().contains("pizza") {
            message.react('\u{1f34d}').ok(); // pineapple
        }

        if message.content.to_lowercase().contains("pineapple") {
            message.react('\u{1f355}').ok(); // pizza
        }

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
            // TODO: update embeds
            if let Some(content) = update.content {
                message.content = content;
            }
            if let Some(attachments) = update.attachments {
                message.attachments = attachments;
            }
            if let Some(edited_timestamp) = update.edited_timestamp {
                message.edited_timestamp = Some(edited_timestamp);
            }
        }
    }

    fn message_delete(&self, _context: Context, channel_id: ChannelId, message_id: MessageId) {
        if CONFIG.discord.log_channels.contains(&channel_id) {
            return;
        }

        if let Ok(Channel::Guild(channel)) = channel_id.to_channel() {
            let channel = channel.read();
            if let Some(message) = MESSAGE_CACHE.read().iter().find(|msg| msg.id == message_id) {
                for log_channel in get_log_channels(channel.guild_id) {
                    if let Err(err) = log_channel.send_message(|msg| {
                        msg.embed(|mut e| {
                            if let Some(embed) = message.embeds.iter().next() {
                                if let Some(ref thumb) = embed.thumbnail {
                                    e = e.thumbnail(&thumb.proxy_url);
                                }
                                if let Some(ref image) = embed.image {
                                    e = e.image(&image.proxy_url);
                                }
                            } else if let Some(attach) = message.attachments.iter().next() {
                                e = e.image(&attach.proxy_url);
                            }
                            e.colour(Colour::RED)
                                .description(format!(
                                    "**Message sent by <@{}> deleted in <#{}>**\n{}",
                                    message.author.id,
                                    channel_id,
                                    message.content_safe()
                                ))
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

    fn guild_member_addition(&self, _context: Context, guild_id: GuildId, mut member: Member) {
        for log_channel in get_log_channels(guild_id) {
            if let Err(err) = log_channel.send_message(|msg| {
                let user = member.user.read();
                msg.embed(|e| {
                    e.colour(Colour::FOOYOO)
                        .description(format!("**<@{}> joined**", user.id))
                        .author(|a| a.name(&user.tag()).icon_url(&user.face()))
                })
            }) {
                warn!("Unable to add member join to log channel: {:?}", err);
            }
        }

        if let Err(err) = crate::CACHE.with(|cache| {
            let uid = member.user.read().id.to_string();
            if let Some(roles) = cache.sticky_roles.get(&uid) {
                for role in roles {
                    if let Err(err) = member.add_role(role) {
                        warn!("Unable to restore a sticky role: {:?}", err);
                    }
                }
            }
        }) {
            warn!("Unable to restore sticky roles: {:?}", err);
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
                    e.colour(Colour::RED)
                        .description(format!("**<@{}> left**", user.id))
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
        let new_user = new_member.user.read();
        let new_nick = new_member.nick.unwrap_or_else(|| new_user.name.clone());
        let sticky_roles: HashSet<RoleId> = new_member
            .roles
            .into_iter()
            .filter(|id| CONFIG.discord.sticky_roles.contains(id))
            .collect();

        if let Err(err) = crate::CACHE.with(|cache| {
            let uid = new_user.id.to_string();
            if sticky_roles.is_empty() {
                cache.sticky_roles.remove(&uid);
            } else {
                cache.sticky_roles.insert(uid, sticky_roles);
            }
        }) {
            warn!("Unable to update sticky roles: {:?}", err);
        }

        if let Some(old_member) = old_member {
            let old_user = old_member.user.read();
            let old_nick = old_member.nick.unwrap_or_else(|| old_user.name.clone());

            if new_nick != old_nick {
                for log_channel in get_log_channels(old_member.guild_id) {
                    if let Err(err) = log_channel.send_message(|msg| {
                        msg.embed(|e| {
                            e.colour(Colour::RED)
                                .description(format!(
                                    "**<@{}> changed their nick**\n{} \u{2192} {}",
                                    new_user.id, old_nick, new_nick
                                ))
                                .author(|a| a.name(&new_user.tag()).icon_url(&new_user.face()))
                        })
                    }) {
                        warn!("Unable to add nick change to log channel: {:?}", err);
                    }
                }
            }
        }
    }
}
