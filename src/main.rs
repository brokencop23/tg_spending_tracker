use tg_spending_tracker::bot::run_bot;
use tg_spending_tracker::db::DB;
use anyhow::Result;


#[tokio::main]
async fn main() -> Result<()> {
    let db = DB::from_memory().await?;
    run_bot(db).await?;
    Ok(())
}
