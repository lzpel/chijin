PATH_DOCS=out/markdown
generate: # 事前準備
	mkdir -p out
	find . -maxdepth 1 -name .gitignore | xargs -IX sed '/^#\s*EOF_DOCKERIGNORE.*/q' X > .dockerignore
test:
	cargo test
deploy: generate # generate out/markdown from examples, then build out/html
	cargo install --root out mdbook --version 0.4.50
	cargo run --example markdown -- $(PATH_DOCS)/SUMMARY.md ./README.md
	./out/bin/mdbook build
publish: # --no-verify skips the full OCCT build verification which takes a very long time
	cargo publish --no-verify
cadrum-occt: generate # native build
	cargo clean
	cargo build --example 01_primitives --release --features source-build 2>&1 | tee out/log.txt # colorはdefaultの一部なのでfeature指定不要
	find target -maxdepth 1 -type d -name 'cadrum*' | xargs -IX sh -c 'tar -czf out/$$(basename X).tar.gz -C $$(dirname X) $$(basename X)'
cadrum-occt-%: # cross build ( = native build in container ) cadrum-occt-aarch64-unknown-linux-gnu cadrum-occt-x86_64-pc-windows-gnu cadrum-occt-x86_64-unknown-linux-gnu
	docker build -f docker/Dockerfile_$(*) -t cadrum-occt-$(*) .
	docker run --rm -v $(PWD)/out/$(*):/src/out cadrum-occt-$(*) make cadrum-occt
check-%: cadrum-occt-% # 
	find out -maxdepth 2 -type f -name '*.tar.gz' | xargs -IX tar -xzf X target
	timeout 300 cargo run --example 01_primitives