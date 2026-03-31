PATH_DOCS=out/markdown
generate: # generate out/markdown from examples, then build out/html
	cargo install --root out mdbook --version 0.4.50
	ls examples/*.rs | xargs -IX basename X .rs | xargs -IX sh -c "mkdir -p $(PATH_DOCS) && cd $(PATH_DOCS) && cargo run --manifest-path ../../Cargo.toml --example X"
	./out/bin/mdbook build
test:
	cargo test --features color
deploy: # --no-verify skips the full OCCT build verification which takes a very long time
	cargo publish --no-verify
deploy-docker:
	docker build . -t lzpel/cadrum && docker push lzpel/cadrum