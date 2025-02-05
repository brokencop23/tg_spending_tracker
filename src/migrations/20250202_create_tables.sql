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
    is_deleted INTEGER DEFAULT 0,
    amount_cent INTEGER
);
