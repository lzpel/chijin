# docker/Makefile — drive all Dockerfile_<target> images locally.
#
# Usage:
#   make -C docker run        # build + run every Dockerfile_*, producing ../out/*.tar.gz
#   make -C docker run-<tgt>  # build + run a single target
#   make -C docker build      # just build the images, do not run them
#   make -C docker list       # show discovered targets
#   make -C docker clean      # remove ../out
#
# Tip: pass `-j` (e.g. `make -C docker -j run`) to build/run targets in parallel.

TARGETS := $(patsubst Dockerfile_%,%,$(wildcard Dockerfile_*))
REPO    := $(abspath ..)
OUT     := $(REPO)/out

.PHONY: list build run clean help \
        $(addprefix build-,$(TARGETS)) \
        $(addprefix run-,$(TARGETS))

help:
	@echo "Targets discovered: $(TARGETS)"
	@echo
	@echo "  make run         build + run every Dockerfile_*"
	@echo "  make run-<tgt>   build + run a single target"
	@echo "  make build       build images only"
	@echo "  make list        print discovered targets"
	@echo "  make clean       rm -rf $(OUT)"

list:
	@echo $(TARGETS)

build: $(addprefix build-,$(TARGETS))
run:   $(addprefix run-,$(TARGETS))

# Generate explicit per-target rules. (Pattern rules with .PHONY are
# unreliable in GNU make — eval'd explicit rules avoid the issue.)
define TARGET_RULES
build-$(1): Dockerfile_$(1)
	docker build -f $$< -t cadrum-prebuild:$(1) .

run-$(1): build-$(1)
	docker run --rm -v "$$(REPO):/src" -v "$$(OUT):/out" cadrum-prebuild:$(1)
endef

$(foreach t,$(TARGETS),$(eval $(call TARGET_RULES,$(t))))

clean:
	rm -rf "$(OUT)"
