#![allow(dead_code)]
use std::collections::HashMap;

use crate::DynResult;
use twitchchat::{
    messages::{self, Commands, Privmsg},
    runner::NotifyHandle,
    AsyncRunner, PrivmsgExt, Status, UserConfig,
};

pub fn get_user_config() -> DynResult<twitchchat::UserConfig> {
    let name = "robbertos_";
    let token = env!("ROBBERTOS_OAUTH");

    let config = UserConfig::builder()
        .name(name)
        .token(token)
        .enable_all_capabilities()
        .build()?;

    Ok(config)
}

pub fn channels_to_join() -> Vec<&'static str> {
    vec!["Just_Robbe_", "crackpotparty"]
}

// a 'main loop'
pub async fn main_loop(mut runner: AsyncRunner) -> DynResult<()> {
    while let Status::Message(msg) = runner.next_message().await? {
        handle_message(msg);
    }

    Ok(())
}

// you can generally ignore the lifetime for these types.
fn handle_message(msg: messages::Commands<'_>) {
    use messages::Commands::Privmsg;
    // All sorts of messages
    if let Privmsg(msg) = msg {
        println!("[{}] {}: {}", msg.channel(), msg.name(), msg.data());
    }
}

pub struct Args<'a, 'b: 'a> {
    pub msg: &'a Privmsg<'b>,
    pub writer: &'a mut twitchchat::Writer,
    pub quit: NotifyHandle,
}

impl<'a, 'b: 'a> Args<'a, 'b> {
    pub fn reply(self, msg: &str) {
        let _ = self.writer.reply(self.msg, msg);
    }
}

pub trait Command: Send + Sync {
    fn handle(&mut self, args: Args<'_, '_>);
}

impl<F> Command for F
where
    F: Fn(Args<'_, '_>),
    F: Send + Sync,
{
    fn handle(&mut self, args: Args<'_, '_>) {
        (self)(args);
    }
}

#[derive(Default)]
pub struct Bot {
    pub commands: HashMap<String, Box<dyn Command>>,
}

impl Bot {
    // add this command to the bot
    pub fn with_command(mut self, name: impl Into<String>, cmd: impl Command + 'static) -> Self {
        self.commands.insert(name.into(), Box::new(cmd));
        self
    }

    // run the bot until its done
    pub async fn run(&mut self, user_config: &UserConfig, channels: &[&str]) -> DynResult<()> {
        // this can fail if DNS resolution cannot happen
        let connector = twitchchat::connector::smol::Connector::twitch()?;

        let mut runner = AsyncRunner::connect(connector, user_config).await?;
        println!("connecting, we are: {}", runner.identity.username());

        for channel in channels {
            println!("joining: {channel}");
            if let Err(err) = runner.join(channel).await {
                eprintln!("error while joining '{channel}': {err}");
            }
        }

        println!("starting main loop");
        self.main_loop(&mut runner).await
    }

    async fn main_loop(&mut self, runner: &mut AsyncRunner) -> DynResult<()> {
        let mut writer = runner.writer();
        let quit = runner.quit_handle();

        loop {
            match runner.next_message().await? {
                Status::Message(Commands::Privmsg(pm)) => {
                    if let Some(cmd) = Self::parse_command(pm.data()) {
                        if let Some(command) = self.commands.get_mut(cmd) {
                            println!("dispatching to: {}", cmd.escape_debug());

                            let args = Args {
                                msg: &pm,
                                writer: &mut writer,
                                quit: quit.clone(),
                            };

                            command.handle(args);
                        }
                    }
                }
                Status::Quit => break,
                _ => {}
            }
        }

        println!("end of main loop");
        Ok(())
    }

    fn parse_command(input: &str) -> Option<&str> {
        if !input.starts_with('!') {
            return None;
        }
        input.split(' ').next()
    }
}
