HELL := /bin/bash
.ONESHELL:
SHELLFLAGS := -eufo pipefail -c

curdir = $(dir $(realpath $(lastword $(MAKEFILE_LIST))))
projdir = $(curdir)
version = $(shell ./ci/get-version.sh)

.PHONY: help
help:
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m\033[0m\n"} /^[a-zA-Z_-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

build-docker-container: ## Build the docker container
	echo "Building docker container with version: $(version)" 
	cd $(projdir)
	docker build -f deploy/Dockerfile -t comsrv:$(version) .
	docker tag comsrv:$(version) comsrv:latest



release: ## Tag release and push
	cd $(ROOT_DIR)
	version=$$(./ci/get-version.sh)
	./ci/update-version.sh
	if [[ $$(git rev-parse --abbrev-ref HEAD) != "main" ]]; then
		echo "Not on main branch"
		exit 1
	fi
	echo "Tagging release $$version"
	git tag -a "release/$$version" -m "Release $$version"
	git push origin "release/$$version"
