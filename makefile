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
ifeq ($(CARGO_BUILD_TARGET),x86_64-pc-windows-gnu)
# mingw gcc のインストール先から lib<name>.a の絶対 path を引く関数。
# 使用例: $(call mingw_lib,libstdc++.a) → /usr/.../libstdc++.a
# -print-file-name= はドライバの library search path から名前解決するだけで、C/C++ 区別はしないので
# gcc 一本で stdc++/gcc/gcc_eh いずれも取れる (g++ でも同じ結果)。コンパイラは parse 時点で
# 存在するので make 関数で OK (対 `$(shell find ...)` は parse 時評価なので cargo build 前に
# 走り空を返す — OCCT lib dir の探索は shell substitution 側で行う、下の recipe を参照)。
mingw_lib = $(shell x86_64-w64-mingw32-gcc -print-file-name=$(1))
endif

cadrum-occt: generate # build occt from source natively
	cargo clean
	cargo build --example 01_primitives --release --features source-build 2>&1 | tee out/log.txt # colorはdefaultの一部なのでfeature指定不要
ifeq ($(CARGO_BUILD_TARGET),x86_64-pc-windows-gnu)
	@# mingw コンテナで作った OCCT の lib dir にコンテナ側 gcc の runtime (libstdc++/libgcc/libgcc_eh) を
	@# libcadrum_* リネームでコピーする。build.rs の scanner が libcadrum_* を自動で -l として拾い、
	@# ホスト側 mingw との GCC バージョン差による ABI 不整合を回避する (#89 対策)。
	@# libTKernel.a の位置から dirname で lib dir を得る。shell substitution `$$(...)` を使うのは
	@# recipe 実行時に評価する必要があるため — make の `$(shell ...)` は recipe 開始時点で
	@# 一括展開されるので cargo build 前に走って空を返してしまう。
	@LIBDIR=$$(find target -maxdepth 6 -type f -name libTKernel.a -path '*cadrum-occt*' | head -n 1 | xargs -r dirname); \
	[ -n "$$LIBDIR" ] || { echo "bundle: OCCT lib dir not found under target/" >&2; exit 1; }; \
	cp -v "$(call mingw_lib,libstdc++.a)" "$$LIBDIR/libcadrum_stdc++.a"; \
	cp -v "$(call mingw_lib,libgcc.a)"    "$$LIBDIR/libcadrum_gcc.a"; \
	cp -v "$(call mingw_lib,libgcc_eh.a)" "$$LIBDIR/libcadrum_gcc_eh.a"
endif
	find target -maxdepth 1 -type d -name 'cadrum*' | xargs -IX sh -c 'tar -czf out/$$(basename X).tar.gz -C $$(dirname X) $$(basename X)'
cadrum-occt-%: # build occt from source in cross ( = native build in container ) cadrum-occt-aarch64-unknown-linux-gnu cadrum-occt-x86_64-pc-windows-gnu cadrum-occt-x86_64-unknown-linux-gnu
	docker build -f docker/Dockerfile_$(*) -t cadrum-occt-$(*) .
	docker run --rm -v $(PWD)/out/$(*):/src/out cadrum-occt-$(*) make cadrum-occt
check-cadrum-occt-%: cadrum-occt-% # varidate builded occt to run binary which is linked with host's code and container's static occt libraries
	find out -maxdepth 2 -type f -name '*.tar.gz' | xargs -IX tar -xzf X -C target
	timeout 300 cargo run --example 01_primitives