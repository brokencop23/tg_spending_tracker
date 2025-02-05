use tg_spending_tracker::bot::run_bot;
use tg_spending_tracker::db::DB;
use anyhow::Result;


#[tokio::main]
async fn main() -> Result<()> {
    let db_path = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "./data/data.db".to_string());
    if !std::fs::exists(&db_path).expect("err") {
        std::fs::File::create(&db_path).expect("DB not created");
    }
    let db = DB::new(&format!("sqlite:{}", &db_path)).await?;
    run_bot(db).await?;
    Ok(())
}
