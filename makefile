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
cadrum-occt-%: generate # cross build ( = native build in container )
	docker build -f docker/Dockerfile_$(*) -t cadrum-occt-$(*) .
	docker run --rm -v $(PWD)/out/$(*):/src/out cadrum-occt-$(*) make cadrum-occt
cadrum-occt-all: generate # cross all build
	make -j3 cadrum-occt-aarch64-unknown-linux-gnu cadrum-occt-x86_64-pc-windows-gnu cadrum-occt-x86_64-unknown-linux-gnu
cadrum-occt: generate # native build (01_primitivesのテストも兼ねる)
	cargo run --example 01_primitives --release --features source-build 2>&1 | tee out/log.txt || true # colorはdefaultの一部なのでfeature指定不要
	echo "is is ok that 01_primitives fails in windows-gnu target." >> out/log.txt
	find target -maxdepth 1 -type d -name 'cadrum*' | xargs -IX sh -c 'tar -czvf out/$$(basename X).tar.gz -C $$(dirname X) $$(basename X)'
	find target -type f -executable -path '*/examples/*' | xargs -IX cp X out/ 2>/dev/null || true
cross-test: # 仮想環境で作ったwindows-gnuプレビルドをホストでテスト、exit 0なら使い物になる
	rm -rf out/x86_64-pc-windows-gnu
	make cadrum-occt-x86_64-pc-windows-gnu
	out/x86_64-pc-windows-gnu/01_primitives.exe