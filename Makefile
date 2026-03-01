.PHONY: fmt test generate-client

fmt:
	cargo fmt

test:
	cargo test

generate-client:
	./scripts/generate-client.sh
