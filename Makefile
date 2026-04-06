.PHONY: run build release test coverage lint fix clean

run:
	clear && cargo run --bin suprim-sql

build:
	cargo build --bin suprim-sql

release:
	cargo build --bin suprim-sql --release

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
	cargo clippy --bin suprim-sql -- -D warnings

fix:
	cargo fix --bin suprim-sql --allow-dirty

clean:
	cargo clean
