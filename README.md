# About

Gsim is a work-in-progress circuit simulator targeting Windows, Linux, Mac and the Web on Desktop.  
This repository contains the graphical circuit editor, the simulation backend can be found at https://github.com/Artentus/gsim.

## Building

To build Gsim you need [Rust](https://www.rust-lang.org/learn/get-started), then run `cargo build --release`.

## Building for the web

To build Gsim for the web you also need [Trunk](https://trunkrs.dev/), which can be installed easily using Cargo: `cargo install --locked trunk`  
Then to build run `trunk build` or to spawn a local dev server run `trunk serve`.
