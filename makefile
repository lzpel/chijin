PATH_DOCS=out/markdown
generate: # prepare for deploy
	mkdir -p out
	find . -maxdepth 1 -name .gitignore | xargs -IX sed '/^#\s*EOF_DOCKERIGNORE.*/q' X > .dockerignore
test: # test all
	cargo test
deploy: generate # generate out/markdown from examples, then build out/html
	cargo install --root out mdbook --version 0.4.50
	cargo run --example markdown -- $(PATH_DOCS)/SUMMARY.md ./README.md
	./out/bin/mdbook build
publish: deploy # publish to crates.io
	cargo publish
cadrum-occt: generate # build occt from source natively
	cargo clean
	cargo build --example 01_primitives --release --features source-build 2>&1 | tee out/log.txt # colorはdefaultの一部なのでfeature指定不要
	find target -maxdepth 1 -type d -name 'cadrum*' | xargs -IX sh -c 'tar -czf out/$$(basename X).tar.gz -C $$(dirname X) $$(basename X)'
cadrum-occt-%: # build occt from source in cross ( = native build in container ) cadrum-occt-aarch64-unknown-linux-gnu cadrum-occt-x86_64-pc-windows-gnu cadrum-occt-x86_64-unknown-linux-gnu
	docker build -f docker/Dockerfile_$(*) -t cadrum-occt-$(*) .
	docker run --rm -v $(PWD)/out/$(*):/src/out cadrum-occt-$(*) make cadrum-occt
check-cadrum-occt-%: cadrum-occt-% # varidate builded occt to run binary which is linked with host's code and container's static occt libraries
	find out -maxdepth 2 -type f -name '*.tar.gz' | xargs -IX tar -xzf X -C target
	timeout 300 cargo run --example 01_primitives