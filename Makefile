# 현재 소스 빌드 결과는 배포본과 다르게 Connection refuse 가 나오는 상황이므로
# 업스트림에서 배포하는 바이너리 파일을 직접 가져와서 패키지 제작
PKG_VER := 0.1.2

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
	@curl -LO https://github.com/KiwiTalk/KiwiTalk/releases/download/v$(PKG_VER)/kiwi-talk_$(PKG_VER)_amd64.deb
	@dpkg-deb -R kiwi-talk_$(PKG_VER)_amd64.deb temp	

install: build
	@echo "Copy binary ..."
	@mkdir -pv usr/bin
	@cp -afv temp/usr .
	@rm -rf temp
	
clean:
	@echo "Cleaning up..."
	@rm -rf temp kiwi-talk_$(PKG_VER)_amd64.deb

uninstall: clean

