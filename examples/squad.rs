// Copyright 2019-present, the HuggingFace Inc. team, The Google AI Language Team and Facebook, Inc.
// Copyright 2019 Guillaume Becquin
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//     http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate failure;
extern crate dirs;

use std::path::PathBuf;
use tch::Device;
use std::env;
use failure::err_msg;
use rust_bert::pipelines::question_answering::{QuestionAnsweringModel, squad_processor};


fn main() -> failure::Fallible<()> {
    //    Resources paths
    let mut home: PathBuf = dirs::home_dir().unwrap();
    home.push("rustbert");
    home.push("distilbert-qa");
    let config_path = &home.as_path().join("config.json");
    let vocab_path = &home.as_path().join("vocab.txt");
    let weights_path = &home.as_path().join("model.ot");

    if !config_path.is_file() | !vocab_path.is_file() | !weights_path.is_file() {
        return Err(
            err_msg("Could not find required resources to run example. \
                          Please run ../utils/download_dependencies_distilbert-qa.py \
                          in a Python environment with dependencies listed in ../requirements.txt"));
    }

//    Set-up Question Answering model
    let device = Device::cuda_if_available();
    let qa_model = QuestionAnsweringModel::new(vocab_path,
                                               config_path,
                                               weights_path, device)?;

//    Define input
    let mut squad_path = PathBuf::from(env::var("squad_dataset")
        .expect("Please set the \"squad_dataset\" environment variable pointing to the SQuAD dataset folder"));
    squad_path.push("dev-v2.0.json");
    let qa_inputs = squad_processor(squad_path);

//    Get answer
    let answers = qa_model.predict(&qa_inputs, 1, 64);
    println!("Sample answer: {:?}", answers.first().unwrap());
    println!("{}", answers.len());
    Ok(())
}