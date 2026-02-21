DOCKER_COMPOSE ?= docker compose
PROJECT ?= hello_nes
ROM_PATH := game_examples/projects/$(PROJECT)/build/$(PROJECT).nes

.PHONY: build clean run shell

build:
	$(DOCKER_COMPOSE) run --rm nesdev make -C projects/$(PROJECT) build

clean:
	$(DOCKER_COMPOSE) run --rm nesdev make -C projects/$(PROJECT) clean

run: build
	cd .. && cargo run -- $(ROM_PATH)

shell:
	$(DOCKER_COMPOSE) run --rm nesdev bash
