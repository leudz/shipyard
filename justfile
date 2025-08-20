test-all:
    cargo test --all-features

test-no-default:
    cargo test --tests --lib --no-default-features

miri: clean _miri

_miri:
    cargo +nightly miri test --tests --lib --no-default-features --features=std

clean:
    cargo clean -p shipyard

clippy $RUSTFLAGS="-D warnings":
    cargo clippy

fmt:
    cargo fmt

doc $RUSTFLAGS="-D warnings":
    cargo doc --all-features --no-deps

build_square_eater:
    cargo build --package square_eater --manifest-path square_eater/Cargo.toml --release --target wasm32-unknown-unknown

move_square_eater:
    rm -f ./square_eater/public/square_eater.wasm
    mv ./target/wasm32-unknown-unknown/release/square_eater.wasm ./square_eater/public/

square_eater: build_square_eater move_square_eater

test: fmt test-all doc miri clippy clean

dev_visualizer $RUSTFLAGS = "--cfg=web_sys_unstable_apis":
    trunk serve visualizer/index.html

visualizer:
    trunk serve visualizer/index.html

rustfmt_errors:
    cargo +nightly fmt -- --config=error_on_line_overflow=true,error_on_unformatted=true

publish version args="":
    cargo release {{version}} {{args}}