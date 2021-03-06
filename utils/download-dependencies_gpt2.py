from transformers import GPT2_PRETRAINED_CONFIG_ARCHIVE_MAP, GPT2_PRETRAINED_MODEL_ARCHIVE_MAP
from transformers.tokenization_gpt2 import PRETRAINED_VOCAB_FILES_MAP
from transformers.file_utils import get_from_cache
from pathlib import Path
import shutil
import os
import numpy as np
import torch
import subprocess

config_path = GPT2_PRETRAINED_CONFIG_ARCHIVE_MAP["gpt2"]
vocab_path = PRETRAINED_VOCAB_FILES_MAP["vocab_file"]["gpt2"]
merges_path = PRETRAINED_VOCAB_FILES_MAP["merges_file"]["gpt2"]
weights_path = GPT2_PRETRAINED_MODEL_ARCHIVE_MAP["gpt2"]

target_path = Path.home() / 'rustbert' / 'gpt2'

temp_config = get_from_cache(config_path)
temp_vocab = get_from_cache(vocab_path)
temp_merges = get_from_cache(merges_path)
temp_weights = get_from_cache(weights_path)

os.makedirs(str(target_path), exist_ok=True)

config_path = str(target_path / 'config.json')
vocab_path = str(target_path / 'vocab.txt')
merges_path = str(target_path / 'merges.txt')
model_path = str(target_path / 'model.bin')

shutil.copy(temp_config, config_path)
shutil.copy(temp_vocab, vocab_path)
shutil.copy(temp_merges, merges_path)
shutil.copy(temp_weights, model_path)

weights = torch.load(temp_weights, map_location='cpu')
nps = {}
for k, v in weights.items():
    nps['transformer.' + k] = np.ascontiguousarray(v.cpu().numpy())
    if k == 'wte.weight':
        nps['lm_head.weight'] = np.ascontiguousarray(v.cpu().numpy())

np.savez(target_path / 'model.npz', **nps)

source = str(target_path / 'model.npz')
target = str(target_path / 'model.ot')

toml_location = (Path(__file__).resolve() / '..' / '..' / 'Cargo.toml').resolve()

subprocess.call(
    ['cargo', 'run', '--bin=convert-tensor', '--manifest-path=%s' % toml_location, '--', source, target])
