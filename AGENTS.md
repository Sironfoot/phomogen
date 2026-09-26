# PhoMoGen - A Photo Mosaic Generator written in Rust

A TUI (Terminal User Interface) CLI application written in Rust that generates photo mosaics using individual frames from video files.

## Project overview

Code is located inside the /src directory with main.rs as the starting point.

This is a CLI terminal app using the Ratatui TUI crate. main.rs contains the TUI's main rendering loop in the `run_app` function. Background tasks need to be performed to avoid blocking the main rendering thread, these tasks are placed in the /src/tasks/ directory and are in the `tasks` module. The user interface rendering can be found in /src/ui/ directory and is in the `ui` module.

Reading video files is performed using ffmpeg which is already installed on the machine, and is called directly from Rust code.

Ignore code inside /examples unless asked to look at it. Files in /images and /videos directories contain sample input videos and output mosaic images, and don't contain any code.

## Formatting

Indentations should be 4 spaces. Write `else {` and `else if {` on their own line.