.PHONY: test vectors attack upgrade audit

test:
	cargo test --workspace -- --test-threads=1

# T-ENC-1: the gate's pinned preimages, then viem's independent recomputation.
vectors:
	cargo test -p constitution-gate --lib enc_vectors
	pnpm --dir tools/vectors-viem install --frozen-lockfile
	pnpm --dir tools/vectors-viem vectors

# Every write input is explicit. No .env sourcing or wallet/RPC defaults; a funded verifier can bill inference.
# The output is a NEW directory; the selected implementation must already be deployed.
DEMO_REQUIRED = BUNDLE CONTRACT IMPLEMENTATION ACCOUNT GATE KEY_FILE NETWORK NEAR_RPC EVM_RPC VERIFIER OUTPUT
WATCH ?= 600
INIT ?= 0x
# Capture the literal value once, then let shell environment expansion form each argument.
# `override` is required for recursively-expanded command-line Make variables containing '$'.
override export BUNDLE := $(value BUNDLE)
override export CONTRACT := $(value CONTRACT)
override export IMPLEMENTATION := $(value IMPLEMENTATION)
override export ACCOUNT := $(value ACCOUNT)
override export GATE := $(value GATE)
override export KEY_FILE := $(value KEY_FILE)
override export NETWORK := $(value NETWORK)
override export NEAR_RPC := $(value NEAR_RPC)
override export EVM_RPC := $(value EVM_RPC)
override export VERIFIER := $(value VERIFIER)
override export OUTPUT := $(value OUTPUT)
override export WATCH := $(value WATCH)
override export INIT := $(value INIT)
override export SEND_RPC := $(value SEND_RPC)
override export DEPLOYMENT := $(value DEPLOYMENT)
attack upgrade:
	@$(foreach name,$(DEMO_REQUIRED),$(if $(strip $($(name))),,$(error Missing $(name); see ARCHITECTURE "Demo runner operation")))
	@cargo run --quiet -p constitution-cli --locked -- demo $@ \
		--bundle "$$BUNDLE" --contract "$$CONTRACT" --impl "$$IMPLEMENTATION" --init "$$INIT" \
		--account "$$ACCOUNT" --gate "$$GATE" --key-file "$$KEY_FILE" --network "$$NETWORK" \
		--near-rpc "$$NEAR_RPC" --evm-rpc "$$EVM_RPC" --verifier "$$VERIFIER" \
		--output "$$OUTPUT" --watch "$$WATCH" $(if $(SEND_RPC),--send-rpc "$$SEND_RPC")

audit:
	@$(foreach name,DEPLOYMENT NEAR_RPC EVM_RPC,$(if $(strip $($(name))),,$(error Missing $(name); provide the deployment record and both read endpoints)))
	@cargo run --quiet -p constitution-cli --locked -- audit --deployment "$$DEPLOYMENT" \
		--near-rpc "$$NEAR_RPC" --evm-rpc "$$EVM_RPC"
