# Square eater

In a world where all round shapes have disappeared.\
You are a lone square trying to get bigger in order to survive the continuous red square attacks.\
When all hope is lost, only the legendary gold square can save you!\
[Play Online](https://leudz.github.io/shipyard/square_eater)

Inspired by Erik Hazzard's [Rectangle Eater](http://erikhazzard.github.io/RectangleEater/).

### Build native

Square eater uses `Macroquad`, you might need to install some dependencies. See [their README](https://github.com/not-fl3/macroquad).

Then when inside the `square_eater` directory you can use regular `cargo` commands to `build` or `run` the game.

### Build WASM

Using [`cargo make`](https://crates.io/crates/cargo-make) so simplify building.\
When in the root directory, build square_eater in release mode, all files will be in the `square_eater/public` directory.

```shell
cargo make square_eater
```

Or if you don't want to use `cargo make`, when in the `square_eater` directory.

```shell
cargo build --release --target wasm32-unknown-unknown
mv ../target/wasm32-unknown-unknown/release/square_eater.wasm ./public/
```

I'm using [`microserver`](https://crates.io/crates/microserver) to make the server.

```shell
microserver square_eater/public
```

Square eater is now accessible at `http://localhost:9090/` (the port might be different if you are using a different tool to make the server).
