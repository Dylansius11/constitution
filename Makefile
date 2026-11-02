.PHONY: test vectors attack upgrade audit

test:
	@echo "test: not implemented" >&2; exit 1

# T-ENC-1: the gate's pinned preimages, then viem's independent recomputation.
vectors:
	cargo test -p constitution-gate --lib enc_vectors
	pnpm --dir tools/vectors-viem install --frozen-lockfile
	pnpm --dir tools/vectors-viem vectors

attack:
	@echo "attack: not implemented" >&2; exit 1

upgrade:
	@echo "upgrade: not implemented" >&2; exit 1

audit:
	@echo "audit: not implemented" >&2; exit 1
