# Analysis pipeline for the Weld agent-scope study.
#
#   make help          list every target
#
# Everything runs inside .venv; `make setup` creates it. Targets that need the
# venv depend on it, so `make run` works from a clean checkout.

.DEFAULT_GOAL := help
SHELL := /bin/bash

VENV    := .venv
PY      := $(VENV)/bin/python
PIP     := $(VENV)/bin/pip
PYTEST  := $(VENV)/bin/pytest

DATA    ?= data
OUT     ?= out
N       ?= 24
SEED    ?= 7

.PHONY: help setup synthetic run run-synthetic test packet merge export clean distclean check

help:  ## Show this help
	@echo "Weld analysis pipeline"
	@echo
	@grep -hE '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk 'BEGIN{FS=":.*?## "}{printf "  \033[1m%-14s\033[0m %s\n", $$1, $$2}'
	@echo
	@echo "Variables: DATA=$(DATA) OUT=$(OUT) N=$(N) SEED=$(SEED)"
	@echo "  e.g. make run DATA=data_real OUT=out_real"

$(VENV)/bin/activate: requirements.txt
	python3 -m venv $(VENV)
	$(PIP) install --quiet --upgrade pip
	$(PIP) install --quiet -r requirements.txt
	@touch $(VENV)/bin/activate

setup: $(VENV)/bin/activate  ## Create the venv and install dependencies
	@echo "venv ready. R for the correctness model: $$(command -v Rscript || echo 'NOT FOUND')"

synthetic: setup  ## Regenerate fake exports into data_synthetic/
	$(PY) synthesize.py --out data_synthetic --n $(N) --seed $(SEED)

run: setup  ## Full analysis: tables, figures, report, R model of record
	$(PY) run_all.py --data $(DATA) --out $(OUT)

run-synthetic: synthetic  ## Regenerate fake data, then analyse it
	$(PY) run_all.py --data data_synthetic --out $(OUT)

test: setup  ## Run the pipeline's regression tests
	$(PYTEST) -q

check: setup  ## Verify the environment before a real run
	@echo "--- python deps ---"
	@$(PY) -c "import pandas,numpy,scipy,statsmodels,matplotlib; print('ok')"
	@echo "--- R (correctness model of record) ---"
	@if command -v Rscript >/dev/null; then \
	  Rscript -e 'cat("ordinal:", requireNamespace("ordinal", quietly=TRUE), " lme4:", requireNamespace("lme4", quietly=TRUE), "\n")'; \
	else \
	  echo "  Rscript NOT FOUND — the CLMM model of record cannot be fitted"; \
	fi
	@echo "--- exports in $(DATA)/ ---"
	@for f in study_participants study_events study_survey; do \
	  if [ -f "$(DATA)/$$f.csv" ]; then echo "  ok      $$f.csv"; \
	  else echo "  MISSING $$f.csv"; fi; done
	@if [ -f "$(DATA)/correctness.csv" ]; then echo "  ok      correctness.csv"; \
	 else echo "  absent  correctness.csv (correctness outputs will be empty)"; fi

packet: setup  ## Build the blinded coding packet from $(DATA)
	$(PY) coding_packet.py --data $(DATA) --out coding

merge: setup  ## Merge coder scores into $(DATA)/correctness.csv
	$(PY) coding_packet.py --merge coding/coder_1.csv coding/coder_2.csv \
	  --out $(DATA)/correctness.csv

export:  ## Pull the three exports from a deploy (needs BASE and TOKEN)
	@test -n "$(BASE)"  || { echo "usage: make export BASE=https://host TOKEN=..."; exit 1; }
	@test -n "$(TOKEN)" || { echo "usage: make export BASE=https://host TOKEN=..."; exit 1; }
	@mkdir -p $(DATA)
	@for ds in participants events survey; do \
	  echo "  fetching $$ds"; \
	  curl -fsS -H "Authorization: Bearer $(TOKEN)" \
	    "$(BASE)/api/v1/study/export?dataset=$$ds" -o "$(DATA)/study_$$ds.csv" \
	    || { echo "  FAILED — is STUDY_EXPORT_TOKEN set on the deploy?"; exit 1; }; \
	done
	@echo "Wrote $(DATA)/study_{participants,events,survey}.csv"

clean:  ## Remove generated outputs (keeps data and venv)
	rm -rf $(OUT) __pycache__ .pytest_cache

distclean: clean  ## Also remove the venv and synthetic data
	rm -rf $(VENV) data_synthetic
