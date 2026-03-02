test:
	cargo test
test-system:
	cargo run --example stretch --features prebuilt --no-default-features
deploy: # --no-verify なしだと bundled feature で OCCT をフルビルドする検証が走り、非常に時間がかかります。
	cargo publish --no-verify