use teloxide::{
    dispatching::{
        dialogue::{InMemStorage, InMemStorageError},
        HandlerExt
    }, prelude::*, utils::command::BotCommands
};
use thiserror::Error;
use crate::db::DB;

type MyDialogue = Dialogue<State, InMemStorage<State>>;


#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    NewCategoryReceiveAlias,
    NewCategoryReceiveName {
        alias: String
    }
}

#[derive(Error, Debug)]
pub enum BotError {
    #[error("request error: {0}")]
    Request(#[from] teloxide::RequestError),
    #[error("db error: {0}")]
    DB(#[from] crate::db::DBError),
    #[error("inmem storage: {0}")]
    InMemStorage(#[from] InMemStorageError)
}


#[derive(BotCommands, Clone)]
#[command(rename_rule="lowercase")]
enum Command {
    #[command(description="help")]
    Help,
    #[command(description="Start the bot")]
    Start,
    #[command(description="List of categories", alias="lc")]
    ListCategory,
    #[command(description="Add category", alias="ac")]
    AddCategory,
}


async fn command_handler(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    cmd: Command,
    db: DB
) -> Result<(), BotError> {
    let chat_id = msg.chat.id;
    match cmd {
        Command::Start => {
            bot.send_message(msg.chat.id, "Hi!").await?;
        }
        Command::ListCategory => {
            let cats = db.get_categories(chat_id).await?;
            let to_sent = match cats.is_empty() {
                true => "No categories created".to_string(),
                false => format!(
                    "Categories \n{}",
                    cats.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("\n")
                )
            };
            bot.send_message(chat_id, to_sent).await?;
        },
        Command::AddCategory => {
            bot.send_message(chat_id, "Enter category alias").await?;
            dialogue.update(State::NewCategoryReceiveAlias).await?;
        },
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?;
        }
    }
    bot.delete_message(chat_id, msg.id).await?;
    Ok(())
}

async fn new_category_get_alias(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    db: DB
) -> Result<(), BotError> {
    let chat_id = msg.chat.id;
    match msg.text() {
        Some(alias) => {
            match db.get_category_by_alias(chat_id, alias.to_string()).await? {
                None => {
                    bot.send_message(chat_id, "Give full name").await?;
                    bot.delete_message(chat_id, msg.id).await?;
                    dialogue.update(State::NewCategoryReceiveName {
                        alias: alias.to_string()
                    }).await?
                },
                Some(row) => {
                    let report = format!("This alias is reserved for {}", row.category.name);
                    bot.send_message(chat_id, report).await?;
                }
            }
        },
        None => {
            bot.send_message(chat_id, "Give an alias for category").await?;
        }
    }
    Ok(())
}

async fn new_category_get_name(
    bot: Bot,
    dialogue: MyDialogue,
    alias: String,
    msg: Message,
    db: DB
) -> Result<(), BotError> {
    let chat_id = msg.chat.id;
    match msg.text() {
        Some(name) => {
            let name = name.to_string();
            let report = format!("Category saved \n\t Alias={alias} \n\t Name={name}");
            db.create_category(chat_id, alias, name).await?;
            bot.send_message(chat_id, report).await?;
            bot.delete_message(chat_id, msg.id).await?;
            dialogue.exit().await?;
        },
        None => {
            bot.send_message(chat_id, "Give a name for category").await?;
        }
    }
    Ok(())
}


pub async fn run_bot(db: DB) -> Result<(), BotError> {
    let bot = Bot::from_env();
    let storage = InMemStorage::<State>::new();
    let handler = Update::filter_message()
        .enter_dialogue::<Message, InMemStorage<State>, State>()
        .branch(
            dptree::entry()
                .filter_command::<Command>()
                .endpoint(command_handler)
        )
        .branch(dptree::case![State::NewCategoryReceiveAlias].endpoint(new_category_get_alias))
        .branch(dptree::case![State::NewCategoryReceiveName { alias }].endpoint(new_category_get_name));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![storage, db.clone()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
