test:
	cargo test
test-system:
	cargo run --example stretch --features OCCT_ROOT --no-default-features
deploy:
	cargo publish --no-verify