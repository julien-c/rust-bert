// Copyright 2020 The Facebook AI Research Team Authors
// Copyright 2020-present, the HuggingFace Inc. team.
// Copyright 2020 Guillaume Becquin
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//     http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::Config;
use tch::{Tensor, nn};
use tch::kind::Kind::{Int64, Float};
use crate::bart::encoder::BartEncoder;
use crate::bart::decoder::BartDecoder;
use tch::nn::{embedding, EmbeddingConfig};
use crate::bart::attention::LayerState;

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize)]
pub enum Activation {
    /// Gaussian Error Linear Unit ([Hendrycks et al., 2016,](https://arxiv.org/abs/1606.08415))
    gelu,
    /// Rectified Linear Unit
    relu,
    /// Swish ([Ramachandran, 2017](https://arxiv.org/abs/1710.05941))
    swish,
    /// Gaussian Error Linear Unit - OpenAI version ([Hendrycks et al., 2016,](https://arxiv.org/abs/1606.08415))
    gelu_new,
    /// Tanh
    tanh,
}

#[derive(Debug, Serialize, Deserialize)]
/// # BART model configuration
/// Defines the BART model architecture (e.g. number of layers, hidden layer size, label mapping...)
pub struct BartConfig {
    pub num_labels: Option<i64>,
    pub activation_function: Option<Activation>,
    pub activation_dropout: f64,
    pub attention_dropout: f64,
    pub classif_dropout: f64,
    pub d_model: i64,
    pub decoder_attention_heads: i64,
    pub decoder_ffn_dim: i64,
    pub decoder_layerdrop: f64,
    pub decoder_layers: i64,
    pub decoder_start_token_id: Option<i64>,
    pub do_sample: bool,
    pub dropout: f64,
    pub early_stopping: bool,
    pub encoder_attention_heads: i64,
    pub encoder_ffn_dim: i64,
    pub encoder_layerdrop: f64,
    pub encoder_layers: i64,
    pub bos_token_id: Option<i64>,
    pub id2label: Option<HashMap<i64, String>>,
    pub label2id: Option<HashMap<String, i64>>,
    pub init_std: f64,
    pub is_decoder: Option<bool>,
    pub is_encoder_decoder: Option<bool>,
    pub length_penalty: f64,
    pub max_length: i64,
    pub max_position_embeddings: i64,
    pub min_length: Option<i64>,
    pub no_repeat_ngram_size: Option<i64>,
    pub num_beams: i64,
    pub num_hidden_layers: i64,
    pub num_return_sequences: i64,
    pub output_attentions: Option<bool>,
    pub output_hidden_states: Option<bool>,
    pub output_past: Option<bool>,
    pub pad_token_id: Option<i64>,
    pub repetition_penalty: f64,
    pub temperature: f64,
    pub top_k: i64,
    pub top_p: f64,
    pub vocab_size: i64,
}

impl Config<BartConfig> for BartConfig {}

fn _prepare_bart_decoder_inputs(pad_token_id: i64,
                                input_ids: &Tensor,
                                decoder_input_ids: Option<&Tensor>,
                                decoder_padding_mask: Option<&Tensor>)
                                -> (Tensor, Option<Tensor>, Option<Tensor>) {
    let decoder_input_ids = match decoder_input_ids {
        Some(value) => value.copy(),
        None => _shift_tokens_right(input_ids, pad_token_id)
    };

    let decoder_padding_mask = match decoder_padding_mask {
        Some(value) => Some(value.eq(0).to_kind(Int64)),
        None => {
            let padding_mask = decoder_input_ids.eq(pad_token_id);
            if i64::from(padding_mask.any()) == 0 {
                None
            } else {
                Some(padding_mask)
            }
        }
    };
    let length = *input_ids.size().last().unwrap();
    let causal_mask = Tensor::empty(&[length, length], (Float, input_ids.device()))
        .fill_(std::f64::NEG_INFINITY)
        .triu(1);

    (decoder_input_ids, decoder_padding_mask, Some(causal_mask))
}


fn _shift_tokens_right(input_ids: &Tensor, pad_token_id: i64) -> Tensor {
    let index_eos: Tensor = input_ids.ne(pad_token_id).sum1(&[-1], true, Int64) - 1;
    let output = input_ids.empty_like().to_kind(Int64);
    output
        .select(1, 0)
        .copy_(&input_ids.gather(1, &index_eos, true).squeeze());
    output
        .slice(1, 1, *output.size().last().unwrap(), 1)
        .copy_(&input_ids.slice(1, 0, *output.size().last().unwrap() - 1, 1));
    output
}

pub struct BartModel {
    encoder: BartEncoder,
    decoder: BartDecoder,
    generation_mode: bool,
    pad_token_id: i64,
}

impl BartModel {
    pub fn new(p: &nn::Path, config: &BartConfig, generation_mode: bool) -> BartModel {
        let p = &(p / "model");
        let pad_token_id = match config.pad_token_id {
            Some(value) => value,
            None => 1
        };
        let embedding_config = EmbeddingConfig { padding_idx: pad_token_id, ..Default::default() };
        let embed_tokens_encoder: nn::Embedding = embedding(p / "shared_encoder",
                                                            config.vocab_size,
                                                            config.d_model,
                                                            embedding_config);
        let embed_tokens_decoder: nn::Embedding = embedding(p / "shared_decoder",
                                                            config.vocab_size,
                                                            config.d_model,
                                                            embedding_config);

        let encoder = BartEncoder::new(p / "encoder", config, embed_tokens_encoder);
        let decoder = BartDecoder::new(p / "decoder", config, embed_tokens_decoder, generation_mode);

        BartModel { encoder, decoder, generation_mode, pad_token_id }
    }

    pub fn forward_t(&mut self,
                     input_ids: &Tensor,
                     attention_mask: Option<&Tensor>,
                     decoder_input_ids: Option<&Tensor>,
                     encoder_outputs: Option<(Tensor, Option<Vec<Tensor>>, Option<Vec<Tensor>>)>,
                     decoder_attention_mask: Option<&Tensor>,
                     train: bool) ->
                     ((Tensor, (Tensor, Option<Tensor>, Option<Vec<(&LayerState, &LayerState)>>), Option<Vec<Tensor>>, Option<Vec<Tensor>>),
                      (Tensor, Option<Vec<Tensor>>, Option<Vec<Tensor>>)) {
        let (decoder_input_ids, decoder_padding_mask, causal_mask) = if self.generation_mode {
            (decoder_input_ids.unwrap().copy(), None, None)
        } else {
            _prepare_bart_decoder_inputs(self.pad_token_id, input_ids, decoder_input_ids, decoder_attention_mask)
        };

        let encoder_outputs = match encoder_outputs {
            Some(value) => value,
            None => {
                self.encoder.forward_t(input_ids, attention_mask, train)
            }
        };

        let decoder_outputs = self.decoder.forward_t(&decoder_input_ids,
                                                     &encoder_outputs.0,
                                                     attention_mask,
                                                     decoder_padding_mask.as_ref(),
                                                     causal_mask.as_ref(),
                                                     train);

        (decoder_outputs, encoder_outputs)
    }
}