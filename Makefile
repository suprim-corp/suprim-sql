.PHONY: run build release test coverage lint fix clean

run:
	clear && cargo run --bin SuprimSQL

build:
	cargo build --bin SuprimSQL

release:
	cargo build --bin SuprimSQL --release

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
