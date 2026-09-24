# 0060. Independent Whisper meeting selection

Status: Accepted 2026-09-16

## What this gives you

Users can keep Parakeet dictation while recording meetings with a downloaded Whisper model.

## Decision

An explicit `speech.meeting_model` identifies a Whisper model. Meeting start and speech selection resolve it independently of `speech.provider`. An empty value retains provider and model inheritance from dictation, including the existing refusal when that provider lacks meeting timestamps.

The settings selector offers Whisper models for either dictation provider. The new-meeting picker writes only the meeting model, preserving dictation selection. No configuration migration is required.

## Verification

Regression coverage checks the settings choices under Parakeet, the meeting picker write, and a daemon meeting start with an explicit Whisper model while Parakeet remains selected for dictation.
