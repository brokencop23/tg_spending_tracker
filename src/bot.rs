use dptree::di;
use teloxide::{
    dispatching::{
        dialogue::{GetChatId, InMemStorage, InMemStorageError},
        HandlerExt
    }, prelude::*, utils::command::BotCommands
};
use thiserror::Error;
use crate::db::{CategoryRow, DB};

type MyDialogue = Dialogue<State, InMemStorage<State>>;


#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    NewCategoryReceiveAlias,
    NewCategoryReceiveName {
        alias: String
    },
    UpdCategoryReceiveAlias,
    UpdCategoryReceiveNewAlias {
        alias: String
    },
    UpdCategoryReceiveNewName {
        alias: String,
        new_alias: String
    },
    NewCostReceiveAlias,
    NewCostReceiveAmount {
        id: i64
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
    #[command(description="New category", alias="nc")]
    AddCategory,
    #[command(description="Update category", alias="uc")]
    UpdateCategory,
    #[command(description="Add cost", alias="cost")]
    AddCost,
    #[command(description="Stat this month", alias="tm")]
    StatThisMonth
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
            bot.send_message(chat_id, "Specify category alias").await?;
            dialogue.update(State::NewCategoryReceiveAlias).await?;
        },
        Command::UpdateCategory => {
            let cats = db.get_categories(chat_id).await?;
            bot.send_message(chat_id, "Specify alias for category to update").await?;
            send_message_with_cats(chat_id, &bot, &cats).await?;
            dialogue.update(State::UpdCategoryReceiveAlias).await?;
        },
        Command::AddCost => {
            let cats = db.get_categories(chat_id).await?;
            bot.send_message(chat_id, "Specify category alias").await?;
            send_message_with_cats(chat_id, &bot, &cats).await?;
            dialogue.update(State::NewCostReceiveAlias).await?;
        },
        Command::StatThisMonth => {
            let stat = db.get_stat_this_month(chat_id).await?;
            bot.send_message(chat_id, stat.to_string()).await?;
        },
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?;
        },
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

async fn send_message_with_cats(
    chat_id: ChatId,
    bot: &Bot,
    cats: &[CategoryRow]
) -> Result<(), BotError> {
    bot.send_message(chat_id, "List of categories available").await?;
    bot.send_message(chat_id, format!(
        "Categories \n{}",
        cats.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("\n")
    )).await?;
    Ok(())
}

async fn upd_category_start(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    db: DB
) -> Result<(), BotError> {
    let chat_id = msg.chat.id;
    let cats = db.get_categories(chat_id).await?;
    match msg.text() {
        Some(alias) => {
            let alias = alias.to_string();
            let n = cats.iter().filter(| i | i.category.alias == alias).collect::<Vec<_>>().len();
            if n == 0 {
                send_message_with_cats(chat_id, &bot, &cats).await?
            } else {
                bot.send_message(chat_id, "Provide new alias").await?;
                dialogue.update(State::UpdCategoryReceiveNewAlias { alias }).await?;
            }
        },
        None => {
            send_message_with_cats(chat_id, &bot, &cats).await?;
        }
    };
    bot.delete_message(chat_id, msg.id).await?;
    Ok(())
}

async fn upd_category_alias(
    bot: Bot,
    dialogue: MyDialogue,
    alias: String,
    msg: Message
) -> Result<(), BotError> {
    let chat_id = msg.chat.id;
    match msg.text() {
        Some(new_alias) => {
            let new_alias = new_alias.to_string();
            bot.send_message(chat_id, "Provide name").await?;
            dialogue.update(State::UpdCategoryReceiveNewName { alias, new_alias }).await?;
        },
        None => {
            bot.send_message(chat_id, "Provide alias name").await?;
        }
    };
    bot.delete_message(chat_id, msg.id).await?;
    Ok(())
}

async fn upd_category_name(
    bot: Bot,
    dialogue: MyDialogue,
    (alias, new_alias): (String, String),
    msg: Message,
    db: DB
) -> Result<(), BotError> {
    let chat_id = msg.chat.id;
    match msg.text() {
        Some(name) => {
            let name = name.to_string();
            db.update_category(chat_id, alias, new_alias, name).await?;
            bot.send_message(chat_id, "Category updated").await?;
            dialogue.exit().await?;
        },
        None => {
            bot.send_message(chat_id, "Provide a name").await?;
        }
    };
    bot.delete_message(chat_id, msg.id).await?;
    Ok(())
}

async fn new_cost_get_alias(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    db: DB
) -> Result<(), BotError> {
    let chat_id = msg.chat.id;
    let cats = db.get_categories(chat_id).await?;
    if let Some(alias) = msg.text() {
        let alias = alias.to_string();
        match cats.iter().filter(|i| i.category.alias == alias).collect::<Vec<_>>().first() {
            Some(cat) => {
                bot.send_message(chat_id, "Specify amount").await?;
                dialogue.update(State::NewCostReceiveAmount { id: cat.id }).await?;
            },
            None => {
                bot.send_message(chat_id, "Specify category alias").await?;
                send_message_with_cats(chat_id, &bot, &cats).await?;
            }
        };
    } else {
        bot.send_message(chat_id, "Specify category alias") .await?;
        send_message_with_cats(chat_id, &bot, &cats).await?;
    }
    bot.delete_message(chat_id, msg.id).await?;
    Ok(())
}

async fn new_cost_get_amount(
    bot: Bot,
    dialogue: MyDialogue,
    id: i64,
    msg: Message,
    db: DB
) -> Result<(), BotError> {
    let chat_id = msg.chat.id;
    if let Some(amount_str) = msg.text() {
        match amount_str.parse::<f64>() {
            Ok(amount) => {
                db.create_cost(id, amount).await?;
                bot.send_message(chat_id, "Created!").await?;
                dialogue.exit().await?;
            },
            Err(_) => {
                bot.send_message(chat_id, "Specify amount").await?;
            }
        };
    }
    bot.delete_message(chat_id, msg.id).await?;
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
        .branch(dptree::case![State::NewCategoryReceiveName { alias }].endpoint(new_category_get_name))
        .branch(dptree::case![State::UpdCategoryReceiveAlias].endpoint(upd_category_start))
        .branch(dptree::case![State::UpdCategoryReceiveNewAlias { alias }].endpoint(upd_category_alias))
        .branch(dptree::case![State::UpdCategoryReceiveNewName { alias, new_alias }].endpoint(upd_category_name))
        .branch(dptree::case![State::NewCostReceiveAlias].endpoint(new_cost_get_alias))
        .branch(dptree::case![State::NewCostReceiveAmount { id }].endpoint(new_cost_get_amount));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![storage, db.clone()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
