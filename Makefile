HELL := /bin/bash
.ONESHELL:
SHELLFLAGS := -eufo pipefail -c

curdir = $(dir $(realpath $(lastword $(MAKEFILE_LIST))))
projdir = $(curdir)
version = $(shell ./ci/get-version.sh)


ifeq ($(OS),Windows_NT)
	venv_exe_dir = $(curdir)/.venv/Scripts
	exe_suffix = .exe
	python = $(venv_exe_dir)/python.exe
	ruff = $(venv_exe_dir)/ruff.exe
	pyright = $(venv_exe_dir)/pyright.exe
else
	venv_exe_dir = $(curdir)/.venv/bin
	exe_suffix =
endif

python = $(venv_exe_dir)/python$(exe_suffix)
ruff = $(venv_exe_dir)/ruff$(exe_suffix)
pyright = $(venv_exe_dir)/pyright$(exe_suffix)


.PHONY: help
help:
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m\033[0m\n"} /^[a-zA-Z_-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)


python-lint: ## Run linters
	cd $(curdir)
	$(ruff) check python/comsrv


python-check: ## Run type check
	cd $(curdir)
	$(pyright) python/comsrv



.PHONY: build-docker-container
build-docker-container: ## Build the docker container
	echo "Building docker container with version: $(version)" 
	cd $(projdir)
	docker build -f deploy/Dockerfile -t comsrv:$(version) .
	docker tag comsrv:$(version) comsrv:latest


.PHONY: release
release: ## Tag release and push
	cd $(projdir)
	version=$$(./ci/get-version.sh --no-pre-release)
	./ci/update-version.sh

	function fail_uncommited_changed() {
		echo "You have uncommitted changes"
		exit 1
	}

	git diff --no-patch --exit-code || fail_uncommited_changed

	if [[ $$(git rev-parse --abbrev-ref HEAD) != "main" ]]; then
		echo "Not on main branch"
		exit 1
	fi

	release_date=$$(grep -m 1 -oE '## \[.*?\] (.*?)' CHANGELOG.md | cut -d '-' -f 2- | xargs)
	if [[ "$$release_date" != "$$(date +%Y-%m-%d)" ]]; then
		echo "Release date is not today"
		exit 1
	fi

	echo "Tagging release $$version"
	git tag -a "release/$$version" -m "Release $$version"
	git push origin "release/$$version"
