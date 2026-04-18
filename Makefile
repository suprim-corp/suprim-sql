.PHONY: run dev build release bundle bundle-sign dmg dmg-sign test coverage lint fix clean

# Run with extensions (license enforced) — same as shipped binary
run:
	clear && cargo run --bin SuprimSQL

# Dev mode — extensions with license bypass for testing
dev:
	clear && cargo run --bin SuprimSQL --features dev-test

build:
	cargo build --bin SuprimSQL

release:
	cargo build --bin SuprimSQL --release

# macOS .app bundle
bundle:
	./scripts/build/macos.sh

# macOS .app bundle + code signing (set CODESIGN_IDENTITY env var)
bundle-sign:
	./scripts/build/macos.sh --sign

# macOS .dmg installer (requires: brew install create-dmg)
dmg:
	./scripts/build/macos.sh --dmg

# macOS .dmg + code signing (full pipeline)
dmg-sign:
	./scripts/build/macos.sh --dmg --sign

test:
	cargo test --lib

test-all:
	cargo test --test postgres_driver_test \
		--test sqlite_driver_test \
		--test mysql_driver_test \
		--test redis_driver_test \
		--test mongodb_driver_test \
		--lib

coverage:
	cargo tarpaulin \
		--test postgres_driver_test \
		--test sqlite_driver_test \
		--test mysql_driver_test \
		--test redis_driver_test \
		--test mongodb_driver_test \
		--lib \
		--exclude-files src/db/mssql.rs \
		--exclude-files src/main.rs

lint:
	cargo clippy --bin SuprimSQL -- -D warnings

fix:
	cargo fix --bin SuprimSQL --allow-dirty

clean:
	cargo clean
