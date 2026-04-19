.PHONY: run clean

run:
	uv sync
	find . -name '*.py' -maxdepth 1 -exec uv run python {} \;

clean:
	rm -f m2_screw.step m2_screw.stl m2_screw.png
