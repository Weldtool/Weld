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

# Which tasks the correctness packet covers: agent | baseline | all.
# `agent` is the half every correctness model actually reads and halves the
# largest coding job, so it is the default; `baseline` builds the remainder
# later, and `make merge` combines rounds rather than replacing them. PSUF keeps
# the two rounds' coder files apart so a second round cannot overwrite the first.
PHASE   ?= agent
PSUF    := $(if $(filter all,$(PHASE)),,_$(PHASE))

# Participants per condition that enter the human-coded set, in enrolment order.
# Recruitment is uncapped because sessions are self-serve and every instrumented
# measure scales for free; hand coding is linear in N and is the study's whole
# researcher cost, so the cap goes here. 0 codes everyone.
CAP     ?= 10

# Share of open-ended responses the second coder also codes. Themes are the one
# single-coded coding -- they are illustrative rather than confirmatory -- so a
# double-coded reliability sample carries the reported kappa.
RELFRAC ?= 0.25

.PHONY: help setup synthetic run run-synthetic test packet merge export clean distclean check \
        packet-agent-errors merge-agent-errors packet-themes merge-themes

help:  ## Show this help
	@echo "Weld analysis pipeline"
	@echo
	@grep -hE '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk 'BEGIN{FS=":.*?## "}{printf "  \033[1m%-14s\033[0m %s\n", $$1, $$2}'
	@echo
	@echo "Variables: DATA=$(DATA) OUT=$(OUT) N=$(N) SEED=$(SEED) PHASE=$(PHASE)"
	@echo "           CAP=$(CAP) per condition, RELFRAC=$(RELFRAC)"
	@echo "  e.g. make run DATA=data_real OUT=out_real"
	@echo "       make packet                  # agent tasks, first $(CAP)/condition"
	@echo "       make packet PHASE=baseline   # the second round, later"
	@echo "       make packet CAP=0            # code everyone (uncapped)"

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
	@if [ -f "$(DATA)/study_agent_turns.csv" ]; then echo "  ok      study_agent_turns.csv"; \
	 else echo "  absent  study_agent_turns.csv (no agent-error coding -> no H4 calibration)"; fi
	@if [ -f "$(DATA)/correctness.csv" ]; then echo "  ok      correctness.csv"; \
	 else echo "  absent  correctness.csv (correctness outputs will be empty)"; fi
	@if [ -f "$(DATA)/agent_errors.csv" ]; then echo "  ok      agent_errors.csv"; \
	 else echo "  absent  agent_errors.csv (H4 calibration falls back to outcome correctness)"; fi
	@if [ -f "$(DATA)/themes.csv" ]; then echo "  ok      themes.csv"; \
	 else echo "  absent  themes.csv (no qualitative themes)"; fi

packet: setup  ## Blinded correctness packet (PHASE=agent|baseline|all)
	$(PY) coding_packet.py --data $(DATA) --phase $(PHASE) --cap $(CAP) --out coding

merge: setup  ## Merge correctness scores into $(DATA)/correctness.csv
	$(PY) coding_packet.py --merge coding/coder_1$(PSUF).csv coding/coder_2$(PSUF).csv \
	  --out $(DATA)/correctness.csv

packet-agent-errors: setup  ## Build the agent-error coding packet (H4). NOT blind.
	$(PY) coding_packet.py --data $(DATA) --kind agent-errors --cap $(CAP) --out coding

merge-agent-errors: setup  ## Merge agent-error codes into $(DATA)/agent_errors.csv
	$(PY) coding_packet.py --kind agent-errors \
	  --merge coding/agent_error_coder_1.csv coding/agent_error_coder_2.csv \
	  --out $(DATA)/agent_errors.csv

packet-themes: setup  ## Build the qualitative (open-ended) coding packet
	$(PY) coding_packet.py --data $(DATA) --kind themes --cap $(CAP) \
	  --reliability-frac $(RELFRAC) --out coding

merge-themes: setup  ## Merge theme codes into $(DATA)/themes.csv
	$(PY) coding_packet.py --kind themes \
	  --merge coding/theme_coder_1.csv coding/theme_coder_2.csv \
	  --out $(DATA)/themes.csv

export:  ## Pull the four exports from a deploy (needs BASE and TOKEN)
	@test -n "$(BASE)"  || { echo "usage: make export BASE=https://host TOKEN=..."; exit 1; }
	@test -n "$(TOKEN)" || { echo "usage: make export BASE=https://host TOKEN=..."; exit 1; }
	@mkdir -p $(DATA)
	@for ds in participants events survey agent_turns; do \
	  echo "  fetching $$ds"; \
	  curl -fsS -H "Authorization: Bearer $(TOKEN)" \
	    "$(BASE)/api/v1/study/export?dataset=$$ds" -o "$(DATA)/study_$$ds.csv" \
	    || { echo "  FAILED — is STUDY_EXPORT_TOKEN set on the deploy?"; exit 1; }; \
	done
	@echo "Wrote $(DATA)/study_{participants,events,survey,agent_turns}.csv"

clean:  ## Remove generated outputs (keeps data and venv)
	rm -rf $(OUT) __pycache__ .pytest_cache

distclean: clean  ## Also remove the venv and synthetic data
	rm -rf $(VENV) data_synthetic
