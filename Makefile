release: clean
	cargo build --release

build: clean
	cargo build

test:
	cargo test

bench:
	cargo bench

clean:
	rm -f lcov.info markrust.profraw

coverage: RUSTC_BOOTSTRAP=1
coverage: RUSTFLAGS=-Zinstrument-coverage
coverage: LLVM_PROFILE_FILE=markrust.profraw
coverage: clean
	cargo build
	cargo test --no-fail-fast
	grcov . -s . \
		--binary-path ./target/debug/ \
		-t lcov \
		--branch \
		--ignore-not-existing \
		-o ./lcov.info

docs:
	cargo rustdoc
