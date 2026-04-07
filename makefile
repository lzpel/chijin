PATH_DOCS=out/markdown
generate: 
	@:
test:
	cargo test
deploy: generate # generate out/markdown from examples, then build out/html
	cargo install --root out mdbook --version 0.4.50
	cargo run --example markdown -- $(PATH_DOCS)/SUMMARY.md ./README.md
	./out/bin/mdbook build
publish: # --no-verify skips the full OCCT build verification which takes a very long time
	cargo publish --no-verify
deploy-docker:
	docker build . -t lzpel/cadrum && docker push lzpel/cadrum
