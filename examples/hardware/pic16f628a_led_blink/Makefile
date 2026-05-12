# SPDX-License-Identifier: GPL-3.0-or-later

PIC ?= picc
SIM ?= pic16-sim
TARGET := pic16f628a
ROOT := ../../..
CFLAGS := -Wall -Wextra -Werror -O2 -I $(ROOT)/include
SRC := main.c
OUT := build/main.hex
MAP := $(OUT:.hex=.map)
FLASH_CMD ?= echo "Configure FLASH_CMD to program"
FLASH_ARGS ?=

$(OUT): $(SRC)
	mkdir -p build
	$(PIC) --target $(TARGET) $(CFLAGS) --map --list-file -o $(OUT) $(SRC)

size: $(SRC)
	mkdir -p build
	$(PIC) --target $(TARGET) $(CFLAGS) --size --memory-report -o $(OUT) $(SRC)

sim: $(OUT)
	$(SIM) $(OUT) --map $(MAP) --max-steps 200000 --print-regs

flash: $(OUT)
	$(FLASH_CMD) $(FLASH_ARGS) $(OUT)

clean:
	rm -rf build

.PHONY: size sim flash clean
