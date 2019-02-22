# NESE

NESE is a NES emulator written in Rust.
It is currently not production quaility, but it can play most games.

# How to run it
Clone the repo then run `cargo run --release <rom_file>`

# Current Status
 - Can play most games. (It can play Battletoads which is considered one of the harder games to emulate.)
 - Emulates sound.
 - Supports Horizontal, Veritical, and 4-Screen Mirroring.
 - Currently supports mappers 0, 1, 2, 3, 4, and 7.

# Things Missing
 - Save state support
 - Other mappers
 - PAL Support
 - WebAssembly version to allow games to be played in the browser
 - Tests (ROM end-to-end tests could probably be added pretty easily)
 - Better desktop version that allows ROMS to be loaded from the GUI,
   save states to be saved and loaded from the GUI, and the screen to be resized.
 - Configurable controls.

# Controls (Currently hard coded)
|   NES  | Emulator    |
| ------ | ----------- |
| A      | A           |
| B      | Z           |
| Start  | Enter       |
| Select | S           |
| Up     | Arrow Up    |
| Right  | Arrow Right |
| Down   | Arrow Down  |
| Left   | Arrow Left  |

# Games that have been tested on this emulator
 - Donkey Kong
 - Super Mario Bros
 - Ice Climbers
 - Arkanoid
 - 1943
 - Mega Man
 - Mega Man 2
 - Mega Man 3
 - Beetlejuice
 - Battletoads
 - Gauntlet
