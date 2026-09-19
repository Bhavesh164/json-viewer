.PHONY: all build release install clean

all: build

build:
	cargo build

release:
	cargo build --release

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
