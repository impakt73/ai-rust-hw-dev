#!/bin/bash
export RUSTFLAGS="-C link-arg=-Tmemory.x -C link-arg=-Tlink.x"
cargo build --release --bin hello_world
