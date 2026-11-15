run test:
    RUSTFLAGS="-C target-cpu=native" cargo run --release -- {{test}}

run-dbg test:
    cargo run -- {{test}}
