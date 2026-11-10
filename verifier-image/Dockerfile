FROM --platform=linux/amd64 python:3.12-slim-bookworm@sha256:623674a14ef93d052bb9b0de4dbf5f28eb578a35263838941cc8aa43213c1487
# apt verifies Debian signed Release metadata and package hashes before installation.
RUN apt-get update && apt-get install -y --no-install-recommends time ca-certificates libgmp10 && rm -rf /var/lib/apt/lists/*
COPY verifier-image/install-tools.py /opt/install-tools.py
RUN python /opt/install-tools.py
ENV PATH="/opt/venv/bin:/opt/node-v24.22.1-linux-x64/bin:/opt/tools/node_modules/.bin:${PATH}"
COPY verifier-image/requirements.lock /opt/requirements.lock
RUN python -m venv /opt/venv && /opt/venv/bin/pip install --no-cache-dir --require-hashes -r /opt/requirements.lock
COPY verifier-image/package.json verifier-image/package-lock.json /opt/tools/
RUN cd /opt/tools && npm ci --ignore-scripts --no-audit --no-fund && npm cache clean --force
COPY contracts/lib/openzeppelin-contracts/contracts /opt/constitution/lib/openzeppelin-contracts/contracts
COPY contracts/lib/openzeppelin-contracts/LICENSE /opt/constitution/lib/openzeppelin-contracts/LICENSE
COPY contracts/lib/openzeppelin-contracts-upgradeable/contracts /opt/constitution/lib/openzeppelin-contracts-upgradeable/contracts
COPY contracts/lib/openzeppelin-contracts-upgradeable/LICENSE /opt/constitution/lib/openzeppelin-contracts-upgradeable/LICENSE
COPY contracts/lib/forge-std/src /opt/constitution/lib/forge-std/src
COPY contracts/lib/forge-std/LICENSE-MIT contracts/lib/forge-std/LICENSE-APACHE /opt/constitution/lib/forge-std/
RUN useradd --uid 10001 --create-home tool && install -d -o tool -g tool /work
COPY spikes/verifier-min /opt/spike
RUN chmod -R a+rX /opt/spike && chmod +x /opt/spike/*.sh
USER 10001:10001
ENV HOME=/work/home
WORKDIR /work
# Static Yices embeds libc cache detection affected by TDX CPUID cache reporting.
# Keep small copies off its faulty non-temporal path; solver logic/bounds are unchanged.
ENV GLIBC_TUNABLES=glibc.cpu.x86_non_temporal_threshold=1048576
CMD ["/bin/sh", "/opt/spike/versions.sh"]
