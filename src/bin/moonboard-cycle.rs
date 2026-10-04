use std::{env, time::Duration};

use anyhow::{Context, Result};
use moonboard_ble::{Hold, HoldRole, MoonBoard};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    let mut holds = vec![Hold {
        index: 0,
        role: HoldRole::Start,
    }];

    let mut args = env::args().skip(1);
    let selector = args.next().context(
        "usage: moonboard-cycle <device name or MAC> [delay-ms]\n\
         example: moonboard-cycle MoonBoard 250",
    )?;
    let delay_ms = args
        .next()
        .map(|value| value.parse())
        .transpose()
        .context("delay-ms must be an integer")?
        .unwrap_or(250);

    let board = MoonBoard::connect(&selector, Duration::from_secs(10)).await?;
    for index in 0..=197 {
        println!("lighting LED {index}");
        board.light(&holds).await?;
        sleep(Duration::from_millis(delay_ms)).await;

        // make fifo list that removes the first element and adds a new foot hold at the end

        holds.push(Hold {
            index,
            role: HoldRole::Foot,
        });
        if holds.len() > 5 {
            holds.remove(0);
        }
    }

    Ok(())
}
