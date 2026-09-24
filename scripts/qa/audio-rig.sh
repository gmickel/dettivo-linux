#!/usr/bin/env bash
# The virtual audio rig (ADR 0011, R4): PipeWire null sinks that real
# capture code reads from, fed with fixture audio on cue.
#
# Usage:
#   scripts/qa/audio-rig.sh up <sink>              # load a null sink, print its monitor source
#   scripts/qa/audio-rig.sh play <sink> <file.wav> # play a fixture into the sink (blocks until done)
#   scripts/qa/audio-rig.sh down <sink>            # unload the sink (mid-run simulates a device swap)
#   scripts/qa/audio-rig.sh list                   # null sinks named dettivo-qa* (the rig's naming convention)
#
# Every failure names the PipeWire module that failed.
set -euo pipefail

cmd="${1:-}"; sink="${2:-}"

need() { command -v "$1" >/dev/null 2>&1 || { echo "audio-rig: $1 is not installed ($2)" >&2; exit 2; }; }
need pactl "pacman -S libpulse"
need pw-play "pacman -S pipewire"

pactl info >/dev/null 2>&1 || { echo "audio-rig: PipeWire is not running (pactl info failed)" >&2; exit 2; }

case "${cmd}" in
    up)
        [[ -n "${sink}" ]] || { echo "audio-rig: up needs a sink name" >&2; exit 2; }
        if ! module="$(pactl load-module module-null-sink "sink_name=${sink}" "sink_properties=device.description=${sink}")"; then
            echo "audio-rig: module-null-sink failed to load for ${sink}" >&2
            exit 1
        fi
        # The monitor source of a null sink is <sink>.monitor.
        for _ in $(seq 1 50); do
            if pactl list short sources | grep -q "^[0-9]*[[:space:]]${sink}.monitor[[:space:]]"; then
                echo "${sink}.monitor"
                exit 0
            fi
            sleep 0.05
        done
        # A sink without its monitor is no use: unload the module this
        # call loaded rather than leave it for the next run to trip over.
        pactl unload-module "${module}" || echo "audio-rig: module-null-sink ${module} failed to unload" >&2
        echo "audio-rig: module-null-sink ${module} loaded but ${sink}.monitor never appeared (unloaded again)" >&2
        exit 1
        ;;
    play)
        file="${3:-}"
        [[ -n "${sink}" && -f "${file}" ]] || { echo "audio-rig: play needs <sink> <file.wav>" >&2; exit 2; }
        pw-play --target "${sink}" "${file}"
        ;;
    down)
        [[ -n "${sink}" ]] || { echo "audio-rig: down needs a sink name" >&2; exit 2; }
        # The sink name is one whole argument of the module line: a
        # substring match would also take dettivo-qa-mic-1234 for -123.
        ids="$(pactl list short modules | awk -v s="sink_name=${sink}" '$2 == "module-null-sink" { for (i = 3; i <= NF; i++) if ($i == s) { print $1; break } }')"
        if [[ -z "${ids}" ]]; then
            echo "audio-rig: no module-null-sink named ${sink} is loaded" >&2
            exit 1
        fi
        for id in ${ids}; do
            pactl unload-module "${id}" || { echo "audio-rig: module-null-sink ${id} failed to unload" >&2; exit 1; }
        done
        ;;
    list)
        pactl list short modules | awk '$2 == "module-null-sink" && index($0, "sink_name=dettivo-qa") { print }'
        ;;
    *)
        echo "usage: $0 up <sink> | play <sink> <file.wav> | down <sink> | list" >&2
        exit 2
        ;;
esac
