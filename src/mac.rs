use anyhow::Result;
use crate::server::server::start;

pub async fn run() -> Result<()> {
    start("", "").await
}