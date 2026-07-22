CC ?= cc
PKG_CONFIG ?= pkg-config

CPPFLAGS += $(shell $(PKG_CONFIG) --cflags webkitgtk-6.0 gstreamer-1.0)
CFLAGS += -std=c17 -O2 -Wall -Wextra -Werror
LDLIBS += $(shell $(PKG_CONFIG) --libs webkitgtk-6.0 gstreamer-1.0)

.PHONY: all clean

all: filament-webkitgtk-probe

filament-webkitgtk-probe: main.c
	$(CC) $(CPPFLAGS) $(CFLAGS) -o $@ $< $(LDLIBS)

clean:
	rm -f filament-webkitgtk-probe
