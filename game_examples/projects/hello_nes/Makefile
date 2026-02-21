ROM_NAME := hello_nes
BUILD_DIR := build
SRC_DIR := src

.PHONY: build clean

build: $(BUILD_DIR)/$(ROM_NAME).nes

$(BUILD_DIR)/main.o: $(SRC_DIR)/main.s
	mkdir -p $(BUILD_DIR)
	ca65 $(SRC_DIR)/main.s -o $(BUILD_DIR)/main.o

$(BUILD_DIR)/$(ROM_NAME).nes: nes.cfg $(BUILD_DIR)/main.o
	ld65 -C nes.cfg $(BUILD_DIR)/main.o -o $(BUILD_DIR)/$(ROM_NAME).nes

clean:
	rm -rf $(BUILD_DIR)
