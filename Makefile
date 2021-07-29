clean:
	rm -f lcov.info

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