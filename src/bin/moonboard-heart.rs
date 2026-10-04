use std::{env, time::Duration};

use anyhow::{Context, Result};
use moonboard_ble::{coordinate_to_index, CoordinateOrder, Hold, HoldRole, IndexBase, MoonBoard};

const HEART: [&str; 9] = [
    "..###.###..",
    ".#...#...#.",
    "#.........#",
    "#.........#",
    ".#.......#.",
    "..#.....#..",
    "...#...#...",
    "....#.#....",
    ".....#.....",
];

fn heart_holds() -> Result<Vec<Hold>> {
    let mut holds = Vec::new();
    for (pattern_row, line) in HEART.into_iter().enumerate() {
        for (column, pixel) in line.bytes().enumerate() {
            if pixel == b'#' {
                let coordinate = format!("{}{}", char::from(b'A' + column as u8), 11 - pattern_row);
                holds.push(Hold {
                    index: coordinate_to_index(
                        &coordinate,
                        CoordinateOrder::ColumnSerpentine,
                        IndexBase::Zero,
                    )?,
                    role: HoldRole::End,
                });
            }
        }
    }
    Ok(holds)
}

#[tokio::main]
async fn main() -> Result<()> {
    let selector = env::args().nth(1).context(
        "usage: moonboard-heart <device name or MAC>\n\
         example: moonboard-heart MoonBoard",
    )?;

    let board = MoonBoard::connect(&selector, Duration::from_secs(10)).await?;
    board.light(&heart_holds()?).await?;
    println!("drew a red heart outline with its tip at F3 / LED 105");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heart_has_expected_outline_and_anchor() {
        let holds = heart_holds().unwrap();
        assert_eq!(holds.len(), 22);
        assert!(holds.iter().any(|hold| hold.index == 105));
    }
}
