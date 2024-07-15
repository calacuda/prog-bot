#!/bin/zsh

cargo add anyhow -F backtrace
cargo add ctrlc
cargo add futures-util
cargo add --path ../../common-lib/
cargo add --path ../../data-types/
cargo add serde_json
cargo add tokio -F full
cargo add tokio-tungstenite
cargo add tracing -F async-await -F log -F log-always
# cargo add serde -F derive
