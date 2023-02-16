# Rust Streaming Server
The goal of this project is to stream (or record) my Windows desktop and send the information to a client.

## Build
This will only build and work on Windows OS

* Make sure you have ffmpeg installed and follow the build instructions for [rust-ac-ffmpeg](https://github.com/angelcam/rust-ac-ffmpeg)

* cargo build --release
* cargo run --release

## Example
* cargo run --release
* cargo run --release -- -d (# of monitor to record)

## Attributions
* Windows API code is adapted from [screenshot-rs](https://github.com/robmikh/screenshot-rs), licensed under the MIT license.
* Referenced code from [sharer](https://github.com/mira-screen-share/sharer), licensed under GPL-3.0 license

