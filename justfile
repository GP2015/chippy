

build:
	cargo build

build_rel:
	RUSTFLAGS="-C target-cpu=native" cargo build --release

run args: build
	./target/debug/chippy {{args}}

run_rel args: build_rel
	./target/release/chippy {{args}}
