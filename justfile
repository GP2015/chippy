build:
	cargo build

build-rel:
	RUSTFLAGS="-C target-cpu=native" cargo build --release

run args: build
	./target/debug/chippy {{args}}

run-rel args: build-rel
	./target/release/chippy {{args}}
