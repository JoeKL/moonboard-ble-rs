use std::{env, time::Duration};

use anyhow::{Context, Result};
use moonboard_ble::{Hold, HoldRole, MoonBoard};

#[tokio::main]
async fn main() -> Result<()> {
    let selector = env::args().nth(1).context(
        "usage: moonboard-green <device name or MAC>\n\
         example: moonboard-green MoonBoard",
    )?;

    let board = MoonBoard::connect(&selector, Duration::from_secs(10)).await?;
    let holds = (0..=197)
        .map(|index| Hold {
            index,
            role: HoldRole::Start,
        })
        .collect::<Vec<_>>();
    board.light(&holds).await?;
    println!("lit LEDs 0 through 197 in green");
    Ok(())
}
