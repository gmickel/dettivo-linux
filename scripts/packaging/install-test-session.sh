#!/usr/bin/env bash
# The --session arm of scripts/packaging/install-test.sh: on a real user
# session the installed units are enabled, socket activation answers
# within 300 ms, the compositor setup writes and checks its snippet, and
# dettivo-osd.service runs (or exits 0 with the panel notice). Sourced by
# install-test.sh after the system install; `out`, `run` and `record` are
# the caller's.
# shellcheck disable=SC2154

check_session() {
  run session_reload "systemctl --user daemon-reload" systemctl --user daemon-reload
  run session_enable "dettivod.socket enabled and started" systemctl --user enable --now dettivod.socket
  systemctl --user stop dettivod.service >/dev/null 2>&1 || true
  local start_ns end_ns ms code=0
  start_ns="$(date +%s%N)"
  dettivo --quiet status ping || code=$?
  end_ns="$(date +%s%N)"
  ms=$(( (end_ns - start_ns) / 1000000 ))
  if [ "$code" -ne 0 ]; then
    record session_activation fail "$code" "dettivo status ping exited $code (journalctl --user -u dettivod)"
  elif ! systemctl --user is-active --quiet dettivod.service; then
    record session_activation fail 1 "ping answered but dettivod.service is not active"
  elif [ "$ms" -gt 300 ]; then
    record session_activation fail 1 "socket activation took $ms ms, over the 300 ms budget"
  else
    record session_activation pass 0 "socket activation answered in $ms ms (budget 300 ms)"
  fi

  # The desktop this session runs: DETTIVO_INSTALL_TEST_COMPOSITOR names
  # it outright; otherwise the omarchy command on PATH says Omarchy (the
  # criterion `dettivo setup omarchy --check` uses for its plugin step)
  # and plain Hyprland is the fallback.
  local compositor="${DETTIVO_INSTALL_TEST_COMPOSITOR:-}"
  if [ -z "$compositor" ]; then
    compositor=hyprland
    if command -v omarchy >/dev/null 2>&1; then compositor=omarchy; fi
  fi
  run session_setup "dettivo setup $compositor wrote the snippet" dettivo setup "$compositor"
  local check setup_code=0
  check="$(dettivo setup "$compositor" --check 2>&1)" || setup_code=$?
  if [ "$setup_code" -eq 0 ]; then
    record session_setup_check pass 0 "dettivo setup $compositor --check: $check"
  else
    record session_setup_check fail "$setup_code" "dettivo setup $compositor --check exited $setup_code: $check"
  fi

  systemctl --user start dettivo-osd.service >"$out/osd.log" 2>&1 || true
  sleep 2
  local state result
  state="$(systemctl --user show -p ActiveState -p ExecMainStatus dettivo-osd.service | tr '\n' ' ')"
  result="$(systemctl --user show -p ExecMainStatus --value dettivo-osd.service)"
  if systemctl --user is-active --quiet dettivo-osd.service; then
    record session_osd pass 0 "dettivo-osd.service is running ($state)"
  elif [ "$result" = 0 ]; then
    record session_osd pass 0 "dettivo-osd.service exited 0, the panel plugin hosts the pill ($state)"
  else
    record session_osd fail 1 "dettivo-osd.service is not running ($state; journalctl --user -u dettivo-osd)"
  fi
}
