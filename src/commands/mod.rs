use serenity::framework::standard::StandardFramework;

mod meta;
mod ranks;

pub fn register(framework: StandardFramework) -> StandardFramework {
    framework
        .command("ping", |cmd| {
            cmd.desc("Replies with a pong.").num_args(0).cmd(meta::ping)
        })
        .command("ranks", |cmd| {
            cmd.desc("Lists all available ranks, as well as the current user's active ones.")
                .num_args(0)
                .guild_only(true)
                .cmd(ranks::list)
        })
        .command("rank", |cmd| {
            cmd.desc("Joins/leaves a rank.")
                .usage("rankname")
                .num_args(1)
                .guild_only(true)
                .cmd(ranks::joinleave)
        })
}
