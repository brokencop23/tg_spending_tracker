use chrono::{DateTime, NaiveDateTime, Utc};
use teloxide::{
    dispatching::{
        dialogue::{InMemStorage, InMemStorageError},
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
    NewCostReceiveAlias {
        amount: f64
    },
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
    #[command(description="Add cost (alias YYYY-MM-DD XX.XX)", alias="cost", parse_with="split")]
    AddCost { alias: String, date: String, amount: f64 },
    #[command(description="Remove last cost", alias="rm")]
    RemoveLastCost,
    #[command(description="Stat this month", alias="stm")]
    StatThisMonth,
    #[command(description="Overall stat in period (YYYY-MM-DD YYYY-MM-DD)", alias="sp", parse_with="split")]
    StatPeriod { date_from: String, date_to: String }, 
}

async fn msg_handler(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    db: DB
) -> Result<(), BotError> {
    let chat_id = msg.chat.id;
    if let Some(text) = msg.text() {
        let mut amount = None;
        let mut cat_id = None;
        for piece in text.split_whitespace() {
            if let Ok(num) = piece.parse::<f64>() {
                amount = Some(num);
            }
            if let Some(cat) = db.get_category_by_alias(chat_id, piece.to_string()).await? {
                cat_id = Some(cat.id);
            }
        }
        match (amount, cat_id) {
            (Some(amount), Some(cat_id)) => {
                db.create_cost(cat_id, amount, None).await?;
                bot.send_message(chat_id, "Added!").await?;
            },
            (None, Some(cat_id)) => {
                bot.send_message(chat_id, "How much?").await?;
                dialogue.update(State::NewCostReceiveAmount { id: cat_id }).await?;
            },
            (Some(amount), None) => {
                bot.send_message(chat_id, "Specify category alias").await?;
                dialogue.update(State::NewCostReceiveAlias { amount }).await?;
            }
            _ => { 
                bot.send_message(chat_id, "/help").await?;
            }
        }
    }
    Ok(())
}

async fn cmd_add_cost(
    bot: Bot,
    db: DB,
    chat_id: ChatId,
    alias: String,
    date: String,
    amount: f64
) -> Result<(), BotError> {
    let cat = match db.get_category_by_alias(chat_id, alias).await? {
        Some(cat) => cat,
        None => {
            bot.send_message(chat_id, "Provide existing category alias").await?;
            return Ok(());
        }
    };
    let dt = match NaiveDateTime::parse_from_str(
        &(date.to_string() + " 00:00:00"),
        "%Y-%m-%d %H:%M:%S"
    ) {
        Ok(dt) => DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc),
        Err(_) => {
            bot.send_message(chat_id, "Provide date in YYYY-MM-DD format").await?;
            return Ok(());
        }
    };
    db.create_cost(cat.id, amount, Some(dt)).await?;
    bot.send_message(chat_id, "Created!").await?;
    Ok(())
}

async fn cmd_list_categories(bot: Bot, db: DB, chat_id: ChatId) -> Result<(), BotError> {
    let cats = db.get_categories(chat_id).await?;
    let to_sent = match cats.is_empty() {
        true => "No categories created".to_string(),
        false => format!(
            "Categories \n{}",
            cats.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("\n")
        )
    };
    bot.send_message(chat_id, to_sent).await?;
    Ok(())
}

async fn cmd_stat_this_month(bot: Bot, db: DB, chat_id: ChatId) -> Result<(), BotError> {
    let stat = db.get_stat_this_month(chat_id).await?;
    bot.send_message(chat_id, stat.to_string()).await?;
    Ok(())
}

async fn cmd_stat_period(
    bot: Bot,
    db: DB,
    chat_id: ChatId,
    date_from: String,
    date_to: String
) -> Result<(), BotError> {
    let df = match NaiveDateTime::parse_from_str(
        &(date_from + " 00:00:00"),
        "%Y-%m-%d %H:%M:%S"
    ) { 
        Ok(df) => DateTime::<Utc>::from_naive_utc_and_offset(df, Utc),
        Err(_) => {
            bot.send_message(chat_id, "Provide date from in YYYY-MM-DD format").await?;
            return Ok(());
        }
    };
    let dt = match NaiveDateTime::parse_from_str(
        &(date_to + " 00:00:00"),
        "%Y-%m-%d %H:%M:%S"
    ) { 
        Ok(dt) => DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc),
        Err(_) => {
            bot.send_message(chat_id, "Provide date to in YYYY-MM-DD format").await?;
            return Ok(());
        }
    };
    let stat = db.get_stat(chat_id, Some(df), Some(dt)).await?;
    bot.send_message(chat_id, stat.to_string()).await?;
    Ok(())
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
            bot.send_message(msg.chat.id, "/help").await?;
        }
        Command::ListCategory => cmd_list_categories(bot, db, chat_id).await?,
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
        Command::AddCost { alias, date, amount } => cmd_add_cost(bot, db, chat_id, alias, date, amount).await?,
        Command::RemoveLastCost => {
            match db.remove_last_cost(chat_id).await? {
                Some(_) => bot.send_message(chat_id, "Removed").await?,
                None => bot.send_message(chat_id, "Nothing to remove").await?
            };
        },
        Command::StatThisMonth => cmd_stat_this_month(bot, db, chat_id).await?,
        Command::StatPeriod { date_from, date_to } => cmd_stat_period(bot, db, chat_id, date_from, date_to).await?,
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?;
        },
    }
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
    Ok(())
}

async fn new_cost_get_alias(
    bot: Bot,
    dialogue: MyDialogue,
    amount: f64,
    msg: Message,
    db: DB
) -> Result<(), BotError> {
    let chat_id = msg.chat.id;
    let cats = db.get_categories(chat_id).await?;
    if let Some(alias) = msg.text() {
        let alias = alias.to_string();
        match cats.iter().filter(|i| i.category.alias == alias).collect::<Vec<_>>().first() {
            Some(cat) => {
                db.create_cost(cat.id, amount, None).await?;
                bot.send_message(chat_id, "Saved").await?;
                dialogue.exit().await?;
            },
            None => {
                send_message_with_cats(chat_id, &bot, &cats).await?;
            }
        };
    } else {
        send_message_with_cats(chat_id, &bot, &cats).await?;
    }
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
                db.create_cost(id, amount, None).await?;
                bot.send_message(chat_id, "Created!").await?;
                dialogue.exit().await?;
            },
            Err(_) => {
                bot.send_message(chat_id, "Specify amount").await?;
            }
        };
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
        .branch(dptree::case![State::NewCategoryReceiveName { alias }].endpoint(new_category_get_name))
        .branch(dptree::case![State::UpdCategoryReceiveAlias].endpoint(upd_category_start))
        .branch(dptree::case![State::UpdCategoryReceiveNewAlias { alias }].endpoint(upd_category_alias))
        .branch(dptree::case![State::UpdCategoryReceiveNewName { alias, new_alias }].endpoint(upd_category_name))
        .branch(dptree::case![State::NewCostReceiveAlias { amount } ].endpoint(new_cost_get_alias))
        .branch(dptree::case![State::NewCostReceiveAmount { id }].endpoint(new_cost_get_amount))
        .branch(Update::filter_message().endpoint(msg_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![storage, db.clone()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
