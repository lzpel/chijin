generate:
	cp -f ../examples/chijin.rs src/chijin.rs
	cargo install --root ../out wasm-pack
	../out/bin/wasm-pack build . -d ../out
	