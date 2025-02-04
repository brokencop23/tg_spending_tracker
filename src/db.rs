use std::fmt::Display;

use sqlx::{
    Row,
    sqlite::{SqlitePool, SqliteRow}
};
use crate::item::{Category};
use teloxide::types::ChatId;
use thiserror::Error;


#[derive(Error, Debug)]
pub enum DBError {
    #[error("failed to connect: {0}")]
    Connection(#[from] sqlx::Error),
    #[error("failed to migrate: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
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

    pub async fn update_category(&self, chat_id: ChatId, id: i64, alias: String, name: String) -> Result<(), DBError> {
        sqlx::query("UPDATE category SET alias=?, name=? WHERE chat_id=? and id=?")
            .bind(alias)
            .bind(name)
            .bind(chat_id.0)
            .bind(id)
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
}

