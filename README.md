# Weegames Demo
This project is the WASM version of [Weegames](https://github.com/yeahross0/weegames) made with macroquad. You can play the demo on [itch.io](https://yeahross.itch.io/weegames).

# Running
To run the executable version run ``cargo run``

To run the web version run

```
cargo build --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/debug/webgames.wasm .
cargo install basic-http-server # If not already installed
basic-http-server .
```

Then open localhost:4000