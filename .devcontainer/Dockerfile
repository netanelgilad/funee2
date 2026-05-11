FROM mcr.microsoft.com/devcontainers/rust:1-1-bullseye

USER vscode

RUN rustup toolchain install stable \
    && rustup default stable \
    && rustup component add rustfmt clippy

RUN curl -fsSL https://opencode.ai/install | bash -s -- --no-modify-path

RUN if ! grep -Fq 'export PATH="$HOME/.opencode/bin:$PATH"' "$HOME/.bashrc"; then \
      printf '\nexport PATH="$HOME/.opencode/bin:$PATH"\n' >> "$HOME/.bashrc"; \
    fi

ENV PATH="/home/vscode/.local/bin:/home/vscode/.opencode/bin:${PATH}"
