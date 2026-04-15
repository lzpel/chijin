# docker/Makefile — prebuilt OCCT tarball 生成の共通入口。
#
# このファイルは 3 つの呼び出し経路から同じ recipe を踏むためのハブ:
#
#   1. 各 Dockerfile_<triple> の `RUN make cadrum-occt-${CARGO_BUILD_TARGET}`
#      (docker build 時、Linux 系 target 全て)
#   2. `.github/workflows/prebuilt.yaml` の windows-msvc ジョブが
#      `windows-2022` runner 上で `make -C docker cadrum-occt-x86_64-pc-windows-msvc`
#   3. ローカル開発での `make -C docker cadrum-occt-<triple>`
#
# どこから叩かれても repo root で cargo を走らせるよう、$(REPO_ROOT) を
# このファイルの位置から絶対パスで解決する。
#
# 使い方:
#   make -C docker cadrum-occt-x86_64-unknown-linux-gnu
#   make -C docker cadrum-occt-aarch64-unknown-linux-gnu
#   make -C docker cadrum-occt-x86_64-pc-windows-gnu
#   make -C docker cadrum-occt-x86_64-pc-windows-msvc

REPO_ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/..)

# build.rs が `target/cadrum-occt-<slug>-<triple>/` に OCCT を install する。
# slug (`V8_0_0_rc5` → `v800rc5`) は build.rs を唯一のソースとし、ここでは
# 計算せず glob で basename を拾う。成果物 tarball は repo root に出力する。
cadrum-occt-%:
	cd $(REPO_ROOT) && cargo build --release --features source-build,color --target $*
	@cd $(REPO_ROOT) && \
	 dir=$$(ls -d target/cadrum-occt-*-$* | head -1); \
	 name=$$(basename $$dir); \
	 tar -czvf $$name.tar.gz -C target $$name; \
	 ls -lh $$name.tar.gz
