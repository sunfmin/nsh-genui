#!/bin/zsh
# Runs the integration tests (tests/live.rs) against DeepSeek with the key in mytokens' `deepseek` profile
# (its ANTHROPIC_AUTH_TOKEN: the same DeepSeek key serves the OpenAI-format endpoint). The key is never printed.
#   scripts/integration.sh [cargo test filter]     GENUI_TEST_MODEL=deepseek-flash GENUI_TEST_THINKING=off|low|default
set -euo pipefail
cd "${0:A:h}/.."
(
  eval "$(mytokens env deepseek)"
  export DEEPSEEK_API_KEY=$ANTHROPIC_AUTH_TOKEN
  unset ANTHROPIC_AUTH_TOKEN ANTHROPIC_BASE_URL
  cargo test --release --test live -- --ignored --nocapture "$@"
)
