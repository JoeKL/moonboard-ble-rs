use std::time::Duration;

use anyhow::{bail, Context, Result};
use btleplug::{
    api::{
        Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter,
        WriteType,
    },
    platform::{Manager, Peripheral},
};
use tokio::time::{sleep, Instant};
use uuid::Uuid;

pub const PIXEL_SINGLE_UUIDS: [Uuid; 4] = [
    Uuid::from_u128(0x713d0003_503e_4c75_ba94_3148f18d941e),
    Uuid::from_u128(0x6e400002_b5a3_f393_e0a9_e50e24dcca9e),
    Uuid::from_u128(0x5b2bf25f_9a69_4a5a_8788_6f3ddcb97fc4),
    Uuid::from_u128(0x7a112233_4455_6677_8899_aabbccddee02),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateOrder {
    RowMajor,
    ColumnMajor,
    ColumnSerpentine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexBase {
    Zero,
    One,
}

/// Converts an `A1` through `K18` coordinate using the selected arithmetic layout.
///
/// MoonBoard setup data can override this relationship with a stored `ledNumber` map,
/// so use the ordering configured by your controller.
pub fn coordinate_to_index(
    coordinate: &str,
    order: CoordinateOrder,
    base: IndexBase,
) -> Result<u16> {
    let coordinate = coordinate.trim();
    let mut chars = coordinate.chars();
    let column = chars
        .next()
        .context("coordinate must contain a column A-K")?
        .to_ascii_uppercase();
    if !(('A'..='K').contains(&column)) {
        bail!("coordinate column must be A-K");
    }

    let row: u16 = chars
        .as_str()
        .parse()
        .context("coordinate row must be 1-18")?;
    if !(1..=18).contains(&row) {
        bail!("coordinate row must be 1-18");
    }

    let column = column as u16 - 'A' as u16;
    let zero_based = match order {
        CoordinateOrder::RowMajor => (row - 1) * 11 + column,
        CoordinateOrder::ColumnMajor => column * 18 + row - 1,
        CoordinateOrder::ColumnSerpentine => {
            column * 18 + if column % 2 == 0 { row - 1 } else { 18 - row }
        }
    };
    Ok(zero_based + u16::from(base == IndexBase::One))
}

#[derive(Clone, Copy, Debug)]
pub enum HoldRole {
    Start,
    Left,
    Right,
    End,
    ScrewOnFoot,
    Match,
    Foot,
    Problem,
}

impl HoldRole {
    const fn token(self) -> char {
        match self {
            Self::Start => 'S',
            Self::Left => 'L',
            Self::Right => 'R',
            Self::End => 'E',
            Self::ScrewOnFoot => 'O',
            Self::Match => 'M',
            Self::Foot => 'F',
            Self::Problem => 'P',
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Hold {
    pub index: u16,
    pub role: HoldRole,
}

pub fn legacy_command(holds: &[Hold]) -> Result<Vec<u8>> {
    if holds.is_empty() {
        bail!("at least one hold is required");
    }

    let tokens = holds
        .iter()
        .map(|hold| format!("{}{}", hold.role.token(), hold.index))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!("l#{tokens}#").into_bytes())
}

pub fn pixel_batch(entries: &[(u16, u16)], first: bool, last: bool) -> Result<Vec<u8>> {
    if entries.is_empty() || entries.len() > u8::MAX as usize {
        bail!("pixelBatch entry count must be 1..255");
    }

    let flags = u8::from(last) | (u8::from(first) << 1);
    let mut packet = Vec::with_capacity(2 + entries.len() * 4);
    packet.extend([flags, entries.len() as u8]);
    for &(index, color_id) in entries {
        packet.extend(index.to_le_bytes());
        packet.extend(color_id.to_le_bytes());
    }
    Ok(packet)
}

pub struct MoonBoard {
    peripheral: Peripheral,
    pixel_single: Characteristic,
    write_type: WriteType,
}

impl MoonBoard {
    pub async fn connect(selector: &str, timeout: Duration) -> Result<Self> {
        let manager = Manager::new().await?;
        let adapter = manager
            .adapters()
            .await?
            .into_iter()
            .next()
            .context("no Bluetooth adapter found")?;

        adapter.start_scan(ScanFilter::default()).await?;
        let deadline = Instant::now() + timeout;
        let selector = selector.to_ascii_lowercase();

        let peripheral = 'found: loop {
            for peripheral in adapter.peripherals().await? {
                let properties = peripheral.properties().await?;
                let name = properties
                    .as_ref()
                    .and_then(|p| p.local_name.as_deref())
                    .unwrap_or_default();
                let address = peripheral.address().to_string();
                if name.to_ascii_lowercase().contains(&selector)
                    || address.to_ascii_lowercase() == selector
                {
                    break 'found peripheral;
                }
            }
            if Instant::now() >= deadline {
                bail!("MoonBoard matching {selector:?} not found");
            }
            sleep(Duration::from_millis(250)).await;
        };

        adapter.stop_scan().await?;
        peripheral.connect().await?;
        peripheral.discover_services().await?;

        let pixel_single = peripheral
            .characteristics()
            .into_iter()
            .find(|characteristic| PIXEL_SINGLE_UUIDS.contains(&characteristic.uuid))
            .context("connected device has no supported pixelSingle characteristic")?;
        let write_type = if pixel_single
            .properties
            .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
        {
            WriteType::WithoutResponse
        } else {
            WriteType::WithResponse
        };

        Ok(Self {
            peripheral,
            pixel_single,
            write_type,
        })
    }

    pub async fn light(&self, holds: &[Hold]) -> Result<()> {
        let command = legacy_command(holds)?;
        for chunk in command.chunks(20) {
            self.peripheral
                .write(&self.pixel_single, chunk, self.write_type)
                .await
                .context("failed to write MoonBoard LED command")?;
            if matches!(self.write_type, WriteType::WithoutResponse) {
                sleep(Duration::from_millis(5)).await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_coordinates_with_selected_layout() {
        assert_eq!(
            coordinate_to_index("B2", CoordinateOrder::RowMajor, IndexBase::Zero).unwrap(),
            12
        );
        assert_eq!(
            coordinate_to_index("B2", CoordinateOrder::ColumnMajor, IndexBase::Zero).unwrap(),
            19
        );
        assert_eq!(
            coordinate_to_index("K18", CoordinateOrder::RowMajor, IndexBase::One).unwrap(),
            198
        );
        assert_eq!(
            coordinate_to_index("A1", CoordinateOrder::ColumnSerpentine, IndexBase::Zero).unwrap(),
            0
        );
        assert_eq!(
            coordinate_to_index("A18", CoordinateOrder::ColumnSerpentine, IndexBase::Zero).unwrap(),
            17
        );
        assert_eq!(
            coordinate_to_index("B18", CoordinateOrder::ColumnSerpentine, IndexBase::Zero).unwrap(),
            18
        );
        assert_eq!(
            coordinate_to_index("B1", CoordinateOrder::ColumnSerpentine, IndexBase::Zero).unwrap(),
            35
        );
        assert_eq!(
            coordinate_to_index("K18", CoordinateOrder::ColumnSerpentine, IndexBase::Zero).unwrap(),
            197
        );
        assert!(coordinate_to_index("L1", CoordinateOrder::RowMajor, IndexBase::Zero).is_err());
        assert!(coordinate_to_index("A19", CoordinateOrder::RowMajor, IndexBase::Zero).is_err());
    }

    #[test]
    fn encodes_legacy_command() {
        let bytes = legacy_command(&[
            Hold {
                index: 12,
                role: HoldRole::Start,
            },
            Hold {
                index: 34,
                role: HoldRole::Right,
            },
        ])
        .unwrap();
        assert_eq!(bytes, b"l#S12,R34#");
    }

    #[test]
    fn encodes_pixel_batch() {
        let bytes = pixel_batch(&[(12, 192), (34, 351)], true, true).unwrap();
        assert_eq!(bytes, [3, 2, 12, 0, 192, 0, 34, 0, 95, 1]);
    }
}
