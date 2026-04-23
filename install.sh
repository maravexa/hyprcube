#!/bin/bash
cargo build --release
sudo install -Dm755 target/release/hyprcube /usr/local/sbin/hyprcube
