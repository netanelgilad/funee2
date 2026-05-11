#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.local/bin:$HOME/.opencode/bin:$PATH"

install_latest_funee() {
  local latest_tag archive_url install_root bin_dir tmp_dir

  latest_tag="$(curl -fsSL https://api.github.com/repos/offloadmywork/funee/releases/latest \
    | node -e 'let data=""; process.stdin.on("data", c => data += c); process.stdin.on("end", () => process.stdout.write(JSON.parse(data).tag_name));')"
  archive_url="https://github.com/offloadmywork/funee/releases/download/${latest_tag}/funee-${latest_tag}-x86_64-unknown-linux-gnu.tar.gz"
  install_root="$HOME/.local/share/funee/${latest_tag}"
  bin_dir="$HOME/.local/bin"
  tmp_dir="$(mktemp -d)"

  mkdir -p "$install_root" "$bin_dir"
  curl -fsSL "$archive_url" | tar -xz -C "$tmp_dir" --strip-components=1
  rm -rf "$install_root/bin" "$install_root/funee-lib"
  mv "$tmp_dir/bin" "$install_root/bin"
  mv "$tmp_dir/funee-lib" "$install_root/funee-lib"
  rm -rf "$tmp_dir"

  cat > "$bin_dir/funee" <<EOF
#!/usr/bin/env bash
FUNEE_LIB_PATH="$install_root/funee-lib/index.ts" exec "$install_root/bin/funee" "\$@"
EOF
  chmod +x "$bin_dir/funee"

  echo "Installed funee ${latest_tag} at $bin_dir/funee"
}

install_latest_funee

bash .devcontainer/opencode-bootstrap.sh

if [ -f "package-lock.json" ]; then
  npm ci
fi

if [ -f "tests/package-lock.json" ]; then
  npm --prefix tests ci
fi

echo "Dev container setup complete."
