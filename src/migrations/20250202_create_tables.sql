CREATE TABLE IF NOT EXISTS category (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER,
    alias STRING,
    name STRING,
    UNIQUE(chat_id, alias)
);

CREATE TABLE IF NOT EXISTS spendings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dt INTEGER,
    category_id INTEGER,
    amount_cent INTEGER
);
