.PHONY: build release install clean test

build:
	cd linux && cargo build

release:
	cd linux && cargo build --release

test:
	cd linux && cargo test

install:
	$(MAKE) -C linux install

clean:
	cd linux && cargo clean
