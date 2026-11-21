# DualLink Controller Studio

**Use a PlayStation DualShock 4 controller with PC games that support Xbox 360 / XInput controllers.**

DualLink Controller Studio is a Windows desktop utility that reads input from a connected game controller, lets you customise how its buttons map to Xbox controls, and can expose that input to games as a virtual Xbox 360 controller.

> **Status:** This project is under development. Controller and game compatibility can vary. Keep the app open while using its virtual controller output.

## Features

- Detects connected gamepads through the `gilrs` input library.
- Reads DualShock 4 controls, including face buttons, D-pad, bumpers, triggers, stick clicks, and analogue sticks.
- Lets you map controller inputs to Xbox 360-style controls.
- Includes an input test view for checking stick and trigger movement.
- Supports multiple mapping profiles, which can be saved locally.
- Creates a virtual Xbox 360 controller through ViGEmBus for games that support XInput.
- Includes setup and compatibility guidance inside the app.

## Requirements

- Windows.
- A compatible game controller (the in-app setup currently focuses on the PlayStation DualShock 4).
- The Rust toolchain, including Cargo, to build from source.
- The signed **ViGEmBus** driver installed separately for virtual Xbox 360 output.

DualLink does not install drivers, modify game files, or guarantee compatibility with every game. Some older DirectInput-only games may not recognise XInput output. Some games, particularly protected online games, may block virtual controllers. Check and follow each game's rules.

## Getting started

### 1. Install Rust

Install Rust using the instructions at [rustup.rs](https://rustup.rs/). On Windows, follow the installer prompts and make sure Cargo is included.

Confirm that Rust and Cargo are available by opening a terminal and running:

```powershell
rustc --version
cargo --version
```

### 2. Get the source code

Clone the repository and open its folder:

```powershell
git clone https://github.com/YOUR-USERNAME/YOUR-REPOSITORY.git
cd YOUR-REPOSITORY
```

Replace the example repository URL and folder name with the actual GitHub repository details.

### 3. Install ViGEmBus

For virtual Xbox controller output, install the signed ViGEmBus driver from the [official ViGEmBus releases](https://github.com/nefarius/ViGEmBus/releases).

Install it yourself and restart Windows if the installer requests it. DualLink does not install this driver for you.

### 4. Build and run DualLink

From the project folder—the folder containing `Cargo.toml`—run:

```powershell
cargo run
```

To create an optimised release build:

```powershell
cargo build --release
```

The executable will be placed under `target\release\` after a successful release build.

> The repository must include its Rust package manifest (`Cargo.toml`) with the dependencies used by the source code. The main source file uses `eframe`/`egui`, `gilrs`, `serde`, `serde_json`, and `vigem_client`.

## Using the app

1. Connect your DualShock 4 controller using USB or Bluetooth.
2. Open DualLink and check that the controller appears in the device area.
3. Open **Button mapping** to choose the Xbox action assigned to each controller input.
4. Open **Profiles** to create and save layouts for different games.
5. Use **Input test** to check the sticks, triggers, and buttons.
6. Open **Compatibility** and enable Xbox output.
7. Leave DualLink running while you play. Start the game and select an Xbox 360 / XInput controller option if the game provides one.

When you close DualLink, its virtual controller is removed.

## Controller profiles and settings

Saved profiles are stored locally on the computer. In the current source version, the Windows profile file is:

```text
%APPDATA%\Padflow\profiles.json
```

The folder keeps the original `Padflow` name so existing saved profiles remain available after the app was renamed to DualLink. Do not delete this folder if you want to keep those profiles.

## Troubleshooting

- **No controller detected:** Check the USB/Bluetooth connection, then reconnect the controller and reopen the app.
- **Xbox output will not start:** Confirm that ViGEmBus is installed correctly. Restart Windows if its installer requested it.
- **The game does not respond:** Make sure Xbox output is active, keep DualLink open, and check the game's controller settings.
- **The game sees two controllers:** Check the game's input settings. The physical controller must remain connected because DualLink reads its input and forwards it to the virtual controller.
- **A game still does not work:** Some games only support DirectInput or block virtual devices. Compatibility is game-dependent.

## Built with

- [Rust](https://www.rust-lang.org/)
- [egui / eframe](https://github.com/emilk/egui) — desktop user interface
- [gilrs](https://github.com/Arvamer/gilrs) — gamepad input
- [Serde](https://serde.rs/) and `serde_json` — profile serialisation
- [vigem_client](https://crates.io/crates/vigem_client) — virtual Xbox controller communication
- [ViGEmBus](https://github.com/nefarius/ViGEmBus) — virtual gamepad driver

## Contributing

Bug reports, suggestions, and contributions are welcome. Please include your Windows version, controller model, steps to reproduce the issue, and any relevant error output when reporting a problem.

## License

No license has been specified yet. Unless a license is added to this repository, all rights are reserved by the copyright holder.
