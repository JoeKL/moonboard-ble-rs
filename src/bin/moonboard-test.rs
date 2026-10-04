use std::{env, time::Duration};

use anyhow::{bail, Context, Result};
use moonboard_ble::{legacy_command, Hold, HoldRole, MoonBoard};

fn parse_hold(value: &str) -> Result<Hold> {
    let (role, index) = value
        .split_once(':')
        .context("hold must use ROLE:INDEX, for example s:12")?;
    let role = match role {
        "s" | "start" => HoldRole::Start,
        "l" | "left" => HoldRole::Left,
        "r" | "right" => HoldRole::Right,
        "e" | "end" => HoldRole::End,
        "o" | "off" => HoldRole::ScrewOnFoot,
        "m" | "match" => HoldRole::Match,
        "f" | "foot" => HoldRole::Foot,
        "p" | "problem" => HoldRole::Problem,
        _ => bail!("unknown role {role:?}"),
    };
    Ok(Hold {
        role,
        index: index.parse().context("LED index must fit in u16")?,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let selector = args.next().context(
        "usage: moonboard-test <device name or MAC> [ROLE:INDEX ...]\n\
         example: moonboard-test MoonBoard s:12 r:34 e:56",
    )?;
    let specs: Vec<_> = args.collect();
    let holds = if specs.is_empty() {
        vec![
            Hold {
                role: HoldRole::Start,
                index: 0,
            },
            Hold {
                role: HoldRole::Right,
                index: 1,
            },
            Hold {
                role: HoldRole::End,
                index: 2,
            },
        ]
    } else {
        specs
            .iter()
            .map(|value| parse_hold(value))
            .collect::<Result<_>>()?
    };

    println!(
        "sending {:?}",
        String::from_utf8(legacy_command(&holds)?).unwrap()
    );
    let board = MoonBoard::connect(&selector, Duration::from_secs(10)).await?;
    board.light(&holds).await?;
    println!("sent");
    Ok(())
}
