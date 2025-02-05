use std::fmt::Display;

use chrono::{DateTime, Datelike, TimeZone, Utc};
use sqlx::{
    Row,
    sqlite::{SqlitePool, SqliteRow}
};
use crate::item::Category;
use teloxide::types::ChatId;
use thiserror::Error;


#[derive(Error, Debug)]
pub enum DBError {
    #[error("failed to connect: {0}")]
    Connection(#[from] sqlx::Error),
    #[error("failed to migrate: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("wrong date format: {0}")]
    DateFormatError(String)
}

pub struct StatCategory {
    category: Category,
    n_items: u64,
    amount: f64
}

impl From<SqliteRow> for StatCategory {
    fn from(row: SqliteRow) -> Self {
        StatCategory {
            category: Category::new(row.get("alias"), row.get("name")),
            n_items: row.get("n"),
            amount: (row.get::<i64,_>("amount") / 100) as f64
        }
    }
}

impl Display for StatCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "-> {}: n={}, amount={}", self.category.name, self.n_items, self.amount)
    }
}

pub struct Stat {
    items: Vec<StatCategory> 
}

impl Stat {

    pub fn new(items: Vec<StatCategory>) -> Self {
        Self { items }
    }

    pub fn n_items(&self) -> u64 {
        self.items.iter().map(|i| i.n_items).sum()
    }

    pub fn amount(&self) -> f64 {
        self.items.iter().map(|i| i.amount).sum()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

impl Display for Stat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cats = self.items.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let report = format!("
        {} \n
        =======================
        Items: {} \t Amount: {}
        ", cats, self.n_items(), self.amount()
        );
        write!(f, "{}", report)
    }
}

pub struct CategoryRow {
    pub id: i64,
    pub chat_id: ChatId,
    pub category: Category
}

impl Display for CategoryRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.category.name, self.category.alias)
    }
}

impl From<SqliteRow> for CategoryRow {
    fn from(row: SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            chat_id: ChatId(row.get("chat_id")),
            category: Category::new(
                row.get("alias"),
                row.get("name")
            )
        }
    }
}

#[derive(Clone)]
pub struct DB {
    conn: SqlitePool
}

impl DB {
    pub async fn new(path: &str) -> Result<Self, DBError> {
        let pool = SqlitePool::connect(path).await?;
        sqlx::migrate!("./src/migrations").run(&pool).await?;
        Ok(Self { conn: pool })
    }

    pub async fn from_memory() -> Result<Self, DBError> {
        Self::new(":memory:").await
    }

    pub async fn get_categories(&self, chat_id: ChatId) -> Result<Vec<CategoryRow>, DBError> {
        let categories = sqlx::query("SELECT id, alias, name, chat_id FROM category WHERE chat_id=? ORDER BY id")
            .bind(chat_id.0)
            .map(| row: SqliteRow | CategoryRow::from(row))
            .fetch_all(&self.conn)
            .await?;
        Ok(categories)
    }

    pub async fn get_category_by_alias(&self, chat_id: ChatId, alias: String) -> Result<Option<CategoryRow>, DBError> {
        let category = sqlx::query("SELECT id, chat_id, alias, name FROM category WHERE chat_id=? AND alias=? LIMIT 1")
            .bind(chat_id.0)
            .bind(alias)
            .map(| row: SqliteRow | CategoryRow::from(row))
            .fetch_optional(&self.conn)
            .await?;
        Ok(category)
    }

    pub async fn update_category(&self, chat_id: ChatId, alias: String, new_alias: String, name: String) -> Result<(), DBError> {
        sqlx::query("UPDATE category SET alias=?, name=? WHERE chat_id=? and alias=?")
            .bind(new_alias)
            .bind(name)
            .bind(chat_id.0)
            .bind(alias)
            .execute(&self.conn)
            .await?;
        Ok(())
    }

    pub async fn create_category(&self, chat_id: ChatId, alias: String, name: String) -> Result<i64, DBError> {
        let id = sqlx::query(
            "INSERT INTO category (chat_id, alias, name) VALUES (?, ?, ?) RETURNING id"
            )
            .bind(chat_id.0)
            .bind(alias)
            .bind(name)
            .fetch_one(&self.conn)
            .await?
            .get::<i64, _>("id");
        Ok(id)
    }

    pub async fn create_cost(&self, category_id: i64, amount: f64) -> Result<i64, DBError> {
        let id = sqlx::query(
            "INSERT INTO spendings (dt, category_id, amount_cent) VALUES (?, ?, ?) RETURNING id"
            )
            .bind(Utc::now().timestamp())
            .bind(category_id)
            .bind((amount * 100.0).round() as i64)
            .fetch_one(&self.conn)
            .await?
            .get::<i64, _>("id");
        Ok(id)
    }

    async fn get_stat(
        &self,
        chat_id: ChatId,
        date_from: Option<DateTime<Utc>>,
        date_to: Option<DateTime<Utc>>
    ) -> Result<Stat, DBError> {

        let mut where_clause = "chat_id=?".to_string();

        if let Some(d) = date_from {
            where_clause = format!("{} AND dt >= {}", where_clause, d.timestamp())
        }

        if let Some(d) = date_to {
            where_clause = format!("{} AND dt < {}", where_clause, d.timestamp())
        }

        let q = format!("
            SELECT
                c.alias AS alias,
                c.name AS name,
                count(0) AS n,
                sum(amount_cent) AS amount
            FROM spendings s
            LEFT JOIN category c
                ON (s.category_id = c.id)
            WHERE {}
            GROUP BY alias, name
        ", where_clause);

        let groups = sqlx::query(&q)
            .bind(chat_id.0)
            .map(| row: SqliteRow | StatCategory::from(row))
            .fetch_all(&self.conn)
            .await?;

        Ok(Stat::new(groups))
    }

    pub async fn get_stat_this_month(&self, chat_id: ChatId) -> Result<Stat, DBError> {
        let now = Utc::now();
        let date_from = Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0).unwrap();

        let next_month = if now.month() == 12 {
            (now.year() + 1, 1)
        } else {
            (now.year(), now.month() + 1)
        };

        let date_to = Utc.with_ymd_and_hms(next_month.0, next_month.1, 1, 0, 0, 0).unwrap();
        self.get_stat(chat_id, Some(date_from), Some(date_to)).await
    }

}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect() {
        let db = DB::from_memory().await;
        assert!(db.is_ok())
    }

    #[tokio::test]
    async fn test_create_category() {
        let db = DB::from_memory().await.unwrap();
        assert_eq!(db.get_categories(ChatId(0)).await.unwrap().len(), 0);
        assert!(db.create_category(ChatId(0), "t".to_string(), "test".to_string()).await.is_ok());
        assert_eq!(db.get_categories(ChatId(0)).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_get_category() {
        let db = DB::from_memory().await.unwrap();
        let id = db.create_category(ChatId(0), "t1".to_string(), "test".to_string()).await;
        assert!(id.is_ok());
    }

    #[tokio::test]
    async fn test_get_category_alias() {
        let db = DB::from_memory().await.unwrap();
        let _ = db.create_category(ChatId(0), "t1".to_string(), "test".to_string()).await;
        let _ = db.create_category(ChatId(0), "t2".to_string(), "test2".to_string()).await;

        if let Some(cat) = db.get_category_by_alias(ChatId(0), "t2".to_string()).await.unwrap() {
            assert_eq!(cat.category.name, "test2")
        }

        match db.get_category_by_alias(ChatId(0), "t3".to_string()).await {
            Ok(None) => assert!(true),
            Ok(Some(_)) => assert!(false),
            Err(_) => assert!(false)
        }
    }

    #[tokio::test]
    async fn test_new_cost() {
        let db = DB::from_memory().await.unwrap();
        let cat_id = db.create_category(ChatId(0), "t1".to_string(), "test".to_string()).await.unwrap();
        assert!(db.create_cost(cat_id, 123.41).await.is_ok());
    }

    #[tokio::test]
    async fn test_stat() {
        let db = DB::from_memory().await.unwrap();

        let cat_id = db.create_category(ChatId(0), "t1".to_string(), "test".to_string()).await.unwrap();
        let _ = db.create_cost(cat_id, 100.0).await.is_ok();
        let _ = db.create_cost(cat_id, 200.0).await.is_ok();
        let _ = db.create_cost(cat_id, 300.0).await.is_ok();

        let cat_id = db.create_category(ChatId(0), "t2".to_string(), "test".to_string()).await.unwrap();
        let _ = db.create_cost(cat_id, 100.0).await.is_ok();
        let _ = db.create_cost(cat_id, 200.0).await.is_ok();
        let _ = db.create_cost(cat_id, 300.0).await.is_ok();
        
        let stat = db.get_stat(ChatId(0), None, None).await.unwrap();
        assert_eq!(stat.n_items(), 6);
        assert_eq!(stat.amount(), 1200.0);
        assert_eq!(stat.len(), 2);
    }

    #[tokio::test]
    async fn test_stat_this_month() {
        let db = DB::from_memory().await.unwrap();

        let cat_id = db.create_category(ChatId(0), "t1".to_string(), "test".to_string()).await.unwrap();
        let _ = db.create_cost(cat_id, 100.0).await.is_ok();
        let _ = db.create_cost(cat_id, 200.0).await.is_ok();
        let _ = db.create_cost(cat_id, 300.0).await.is_ok();

        let cat_id = db.create_category(ChatId(0), "t2".to_string(), "test".to_string()).await.unwrap();
        let _ = db.create_cost(cat_id, 100.0).await.is_ok();
        let _ = db.create_cost(cat_id, 200.0).await.is_ok();
        let _ = db.create_cost(cat_id, 300.0).await.is_ok();
        
        let stat = db.get_stat_this_month(ChatId(0)).await.unwrap();
        assert_eq!(stat.n_items(), 6);
        assert_eq!(stat.amount(), 1200.0);
        assert_eq!(stat.len(), 2);
    }
}
