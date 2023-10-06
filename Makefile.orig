all: build install

prepare:
	@echo "Checking enviroment ..."
ifneq ("$(wildcard $(HOME)/.cargo/bin/rustc)","")
	@echo "Found rust enviroment"
else
	@echo "Install rust ..."
	@curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y
endif

build: prepare
	@echo "Build ..."
	@export PATH="$HOME/.cargo/bin:$PATH"	
	@cargo build --release --verbose

install: build
	@echo "Copy binary ..."
	@mkdir -pv usr/bin
	@cp -afv target/release/kiwi-talk-app usr/bin/kiwi-talk

clean:
	@echo "Cleaning up..."
	@rm -rf target usr/bin

uninstall: clean

