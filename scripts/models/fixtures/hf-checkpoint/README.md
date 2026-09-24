# A Hugging Face export's shape

`config.json` and `model.safetensors` are placeholders with the file names a fused export carries, so `scripts/models/convert-polish-finetune.sh --dry-run --source scripts/models/fixtures/hf-checkpoint` can validate its toolchain and paths in CI without a checkpoint (`just polish-convert-check`). The full conversion runs on a development machine against a real export ([docs/polish-models.md](../../../../docs/polish-models.md)).
