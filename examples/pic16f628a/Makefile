PIC ?= picc
TARGET := pic16f628a
ROOT := ../..
CFLAGS := -Wall -Wextra -Werror -O2 -I $(ROOT)/include
SRC ?= blink.c
OUT ?= build/$(basename $(notdir $(SRC))).hex
FLASH_CMD ?= echo "Configure FLASH_CMD to program"

$(OUT): $(SRC)
	mkdir -p build
	$(PIC) --target $(TARGET) $(CFLAGS) -o $(OUT) $(SRC)

clean:
	rm -rf build

flash: $(OUT)
	$(FLASH_CMD) $(OUT)

.PHONY: clean flash
