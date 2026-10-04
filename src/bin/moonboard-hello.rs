use std::{env, time::Duration};

use anyhow::{Context, Result};
use moonboard_ble::{coordinate_to_index, CoordinateOrder, Hold, HoldRole, IndexBase, MoonBoard};
use tokio::time::sleep;

const HELLO: [(&str, [&str; 5]); 5] = [
    ("H", ["#.#", "#.#", "###", "#.#", "#.#"]),
    ("E", ["###", "#..", "##.", "#..", "###"]),
    ("L", ["#..", "#..", "#..", "#..", "###"]),
    ("L", ["#..", "#..", "#..", "#..", "###"]),
    ("O", ["###", "#.#", "#.#", "#.#", "###"]),
];

fn led_index(column: usize, row: usize) -> Result<u16> {
    coordinate_to_index(
        &format!("{}{}", char::from(b'A' + column as u8), row + 1),
        CoordinateOrder::ColumnSerpentine,
        IndexBase::Zero,
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let selector = args.next().context(
        "usage: moonboard-hello <device name or MAC> [letter-ms]\n\
         example: moonboard-hello MoonBoard 700",
    )?;
    let letter_ms = args
        .next()
        .map(|value| value.parse())
        .transpose()
        .context("letter-ms must be an integer")?
        .unwrap_or(700);

    let board = MoonBoard::connect(&selector, Duration::from_secs(10)).await?;
    for (letter, pattern) in HELLO {
        let mut holds = Vec::new();
        for (pattern_row, line) in pattern.into_iter().enumerate() {
            for (pattern_column, pixel) in line.bytes().enumerate() {
                if pixel == b'#' {
                    holds.push(Hold {
                        index: led_index(pattern_column + 4, 11 - pattern_row)?,
                        role: HoldRole::Right,
                    });
                }
            }
        }

        println!("{letter}");
        board.light(&holds).await?;
        sleep(Duration::from_millis(letter_ms)).await;
        board
            .light(&[Hold {
                index: 0,
                role: HoldRole::ScrewOnFoot,
            }])
            .await?;
        sleep(Duration::from_millis(100)).await;
    }
    Ok(())
}
