.PHONY: all build release package install test clean

all: build

build:
	cargo build

release:
	cargo build --release

package: release
	@mkdir -p target/release/staging
	@cp target/release/jsonviewer target/release/staging/
	@cp resources/jsonviewer.desktop target/release/staging/
	@cp resources/AppIcon.png target/release/staging/jsonviewer.png
	@cp resources/AppIcon.png target/release/staging/
	@cp target/release/jsonviewer target/release/jsonviewer-linux-x86_64
	@tar -czf target/release/jsonviewer-linux-x86_64.tar.gz -C target/release/staging .
	@rm -rf target/release/staging
	@echo "Release archive created: target/release/jsonviewer-linux-x86_64.tar.gz"
	@echo "Standalone binary created: target/release/jsonviewer-linux-x86_64"

test:
	cargo test

install: release
	install -d $(DESTDIR)/usr/local/bin
	install -m 755 target/release/jsonviewer $(DESTDIR)/usr/local/bin/jsonviewer
	install -d $(DESTDIR)/usr/share/applications
	install -m 644 resources/jsonviewer.desktop $(DESTDIR)/usr/share/applications/jsonviewer.desktop
	install -d $(DESTDIR)/usr/share/icons/hicolor/512x512/apps
	install -m 644 resources/AppIcon.png $(DESTDIR)/usr/share/icons/hicolor/512x512/apps/jsonviewer.png
	install -d $(DESTDIR)/usr/share/pixmaps
	install -m 644 resources/AppIcon.png $(DESTDIR)/usr/share/pixmaps/jsonviewer.png
	-update-desktop-database $(DESTDIR)/usr/share/applications 2>/dev/null || true
	-gtk-update-icon-cache -f -t $(DESTDIR)/usr/share/icons/hicolor 2>/dev/null || true

clean:
	cargo clean

