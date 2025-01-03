use crate::include::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, Mutex},
};
use twitchchat::{messages::Privmsg, PrivmsgExt as _};

mod include;
type DynResult<T> = Result<T, Box<dyn Error>>;

const START_COINS: u64 = 1000;
const OWNER_NAME: &str = "just_robbe_";

#[derive(Serialize, Deserialize, Default)]
struct BotSaveData {
    coins: HashMap<String, u64>,
}

// clones the objects passed in to it,
macro_rules! enclose {
    ( ($( $x:ident ),*) $y:expr ) => {
        {
            $(let $x = $x.clone();)*
            $y
        }
    };
}

impl BotSaveData {
    fn load() -> Self {
        let coin_file = std::fs::read_to_string("coin_store.json").unwrap();
        let save_data: Self = serde_json::from_str(&coin_file).unwrap_or_default();
        save_data
    }

    fn reload(&mut self) {
        *self = Self::load();
    }

    fn save(&self) {
        if let Ok(save_str) = serde_json::to_string(&self) {
            let _ = std::fs::write("coin_store.json", save_str); // ignore if it errors
        };
    }

    fn gamble(&mut self, msg: &Privmsg) -> Option<String> {
        let user_name = msg.name().to_owned();

        let mut user_coins = *self.coins.get(&user_name).unwrap_or(&START_COINS);

        if user_coins == 0 {
            return Some("you are broke, dont gamble".to_owned());
        }

        // get the first element after the command
        let message_content = msg.data().split_whitespace().nth(1)?;
        let ammount = &message_content.parse().ok()?;

        if *ammount == 0 {
            return Some("cant gamble 0 coins :(".to_owned());
        }

        let reply = if rand::thread_rng().gen_bool(0.5) {
            user_coins = user_coins.saturating_add(*ammount);
            format!("you WON! and now have {user_coins} coins")
        } else {
            user_coins = user_coins.saturating_sub(*ammount);
            format!("you lost {ammount} coins and now have {user_coins} total")
        };

        self.coins.insert(user_name, user_coins);
        self.save();
        Some(reply)
    }

    fn set_coins(&mut self, msg: &Privmsg) -> Option<()> {
        let message_content = msg.data().split_whitespace().nth(1)?;
        let ammount = &message_content.parse().ok()?;

        self.coins.insert(msg.name().to_owned(), *ammount);

        Some(())
    }

    fn add_coins(&mut self, msg: &Privmsg) -> Option<()> {
        let user_name = msg.name().to_owned();
        let message_content = msg.data().split_whitespace().nth(1)?;
        let ammount = &message_content.parse().ok()?;

        if let Some(coins) = self.coins.get_mut(&user_name) {
            *coins += ammount;
        } else {
            self.coins.insert(user_name, *ammount);
        }

        Some(())
    }
}

fn main() -> DynResult<()> {
    let user_config = get_user_config()?;
    let channels = channels_to_join();
    let save_data = Arc::new(Mutex::new(BotSaveData::load()));

    let mut bot = Bot::default()
        .with_command(
            "!gamble",
            enclose! { (save_data) move |args: Args| {

                let mut bot_data = if let Ok(v) = save_data.lock() { v } else {
                       save_data.clear_poison();
                       let mut lock = save_data.lock().unwrap();
                       lock.reload();
                       lock
                   };

                let Some(reply) = bot_data.gamble(args.msg) else {
                    return;
                };

                args.reply(&reply);
            }},
        )
        .with_command(
            "!set_coins",
            enclose! { (save_data) move |args: Args| {
                if args.msg.name() != OWNER_NAME {
                    return;
                }

                let mut bot_data = save_data.lock().unwrap(); // TODO: handle poison
                bot_data.set_coins(args.msg);
            }},
        )
        .with_command(
            "!add_coins",
            enclose! { (save_data) move |args: Args| {
                if args.msg.name() != OWNER_NAME {
                    return;
                }
                let mut bot_data = save_data.lock().unwrap(); // TODO: handle poison
                bot_data.add_coins(args.msg);
            }},
        )
        .with_command("!topic", |args: Args| {
            let messages: Vec<_> = include_str!("./themes").lines().collect();
            let rng = rand::thread_rng().gen_range(0..messages.len());

            let output = messages[rng];
            args.writer.reply(args.msg, output).unwrap();
        })
        .with_command("!quote", |args: Args| {
            let messages: Vec<_> = include_str!("./quotes").lines().collect();
            let rng = rand::thread_rng().gen_range(0..messages.len());

            let output = messages[rng];
            args.writer.reply(args.msg, output).unwrap();
        });

    smol::block_on(async move { bot.run(&user_config, &channels).await })
}
