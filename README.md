# moonboard-ble

Small Rust library and test sender for the MoonBoard `pixelSingle` BLE protocol recovered from app 1.3.68.

## Run

Turn on and stay close to the board, then run:

```sh
cargo run --bin moonboard-test -- MoonBoard s:12 r:34 e:56
```

The first argument is a case-insensitive substring of the advertised device name, or an exact Bluetooth address. Holds use `ROLE:INDEX`:

```text
s start   l left   r right   e end
f foot    m match  p problem o off
```

With no hold arguments, the program sends `l#S0,R1,E2#`. CLI role arguments are case-insensitive; legacy 2019 controllers require uppercase role tokens on the wire for colors.

Cycle once through LED indices 1 through 197, waiting 250 ms between LEDs:

```sh
cargo run --bin moonboard-cycle -- MoonBoard 250
```

The optional final argument controls the delay in milliseconds.

Light all LEDs green:

```sh
cargo run --bin moonboard-green -- MoonBoard
```

Display H, E, L, L, O one letter at a time:

```sh
cargo run --bin moonboard-hello -- MoonBoard 700
```

Draw a red heart outline with its tip at `F3` / LED `105`:

```sh
cargo run --bin moonboard-heart -- MoonBoard
```

Play Snake on the 11×18 LED grid:

```sh
cargo run --bin moonboard-snake -- MoonBoard 350
```

Use the arrow keys or WASD to turn and `q` or Escape to quit. Crossing any edge wraps the snake to the opposite side. The optional final argument is the movement interval in milliseconds. The green LED is the head, the blue LED trail is the body, and the red LED is food. The terminal also displays the board and score.

The game uses the confirmed alternating-column wiring: `A1..A18`, `B18..B1`, `C1..C18`, etc. These correspond to zero-based physical rows `A0..A17`, `B17..B0`, and LED indices `0..=197`.

`INDEX` is the setup's stored `ledNumber`, not a calculation from a label such as `A1`. The snake binary explicitly uses the confirmed alternating-column layout.

On Linux, BlueZ must be running and the user must have permission to access Bluetooth over D-Bus.

## Library

```rust
use moonboard_ble::{Hold, HoldRole, MoonBoard};
use std::time::Duration;

# async fn example() -> anyhow::Result<()> {
let board = MoonBoard::connect("MoonBoard", Duration::from_secs(10)).await?;
board.light(&[
    Hold { index: 12, role: HoldRole::Start },
    Hold { index: 34, role: HoldRole::Right },
]).await?;
# Ok(())
# }
```

Pure encoders are also available as `legacy_command` and `pixel_batch`.

Coordinate conversion supports every arithmetic layout explicitly:

```rust
use moonboard_ble::{coordinate_to_index, CoordinateOrder, IndexBase};

let row_major = coordinate_to_index("K18", CoordinateOrder::RowMajor, IndexBase::Zero)?;
assert_eq!(row_major, 197);

let column_major = coordinate_to_index("B2", CoordinateOrder::ColumnMajor, IndexBase::One)?;
assert_eq!(column_major, 20);

let snake = coordinate_to_index(
    "B18",
    CoordinateOrder::ColumnSerpentine,
    IndexBase::Zero,
)?;
assert_eq!(snake, 18);
# Ok::<(), anyhow::Error>(())
```

Choose the ordering and base used by your controller; the official app normally uses its downloaded `ledNumber` mapping.
