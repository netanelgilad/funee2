#!/usr/bin/env bash
set -euo pipefail

OPENCODE_CONFIG_DIR="${HOME}/.config/opencode"
OPENCODE_DATA_DIR="${HOME}/.local/share/opencode"

mkdir -p "${OPENCODE_CONFIG_DIR}" "${OPENCODE_DATA_DIR}"

# Optional: seed provider auth from a Codespaces secret.
# Secret format should be the full JSON content of auth.json.
if [ -n "${OPENCODE_AUTH_JSON:-}" ]; then
  printf '%s' "${OPENCODE_AUTH_JSON}" > "${OPENCODE_DATA_DIR}/auth.json"
  chmod 600 "${OPENCODE_DATA_DIR}/auth.json"
  echo "OpenCode auth.json seeded from OPENCODE_AUTH_JSON."
fi

# Optional: seed global OpenCode config from a Codespaces secret.
# Secret format should be full JSON config content.
if [ -n "${OPENCODE_GLOBAL_CONFIG_JSON:-}" ]; then
  printf '%s' "${OPENCODE_GLOBAL_CONFIG_JSON}" > "${OPENCODE_CONFIG_DIR}/opencode.json"
  chmod 600 "${OPENCODE_CONFIG_DIR}/opencode.json"
  echo "OpenCode global config seeded from OPENCODE_GLOBAL_CONFIG_JSON."
fi

# Optional: persist preferred default model.
# Secret format should be a model id string, eg openai/gpt-5.3.
if [ -n "${OPENCODE_MODEL:-}" ]; then
  python - "${OPENCODE_CONFIG_DIR}/opencode.json" "${OPENCODE_MODEL}" <<'PY'
import json
import os
import sys

config_path = os.path.expanduser(sys.argv[1])
model = sys.argv[2]

config = {}
if os.path.exists(config_path):
    with open(config_path, "r", encoding="utf-8") as f:
        config = json.load(f)

config["model"] = model

os.makedirs(os.path.dirname(config_path), exist_ok=True)
with open(config_path, "w", encoding="utf-8") as f:
    json.dump(config, f, indent=2)
    f.write("\n")
PY
  chmod 600 "${OPENCODE_CONFIG_DIR}/opencode.json"
  echo "OpenCode model set from OPENCODE_MODEL (${OPENCODE_MODEL})."
fi

if [ -z "${OPENCODE_AUTH_JSON:-}" ] && [ -z "${OPENCODE_GLOBAL_CONFIG_JSON:-}" ] && [ -z "${OPENCODE_MODEL:-}" ]; then
  echo "OpenCode bootstrap: no auth/config/model secrets found (this is okay)."
fi
