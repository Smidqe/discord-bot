use crate::CONFIG;
use serenity::model::prelude::*;

pub fn can_talk_in(channel_id: ChannelId) -> bool {
    !CONFIG.discord.channel_blacklist.contains(&channel_id)
        && (CONFIG.discord.channel_whitelist.is_empty()
            || CONFIG.discord.channel_whitelist.contains(&channel_id))
}

pub fn can_respond_to(message: &Message) -> bool {
    can_talk_in(message.channel_id)
}
