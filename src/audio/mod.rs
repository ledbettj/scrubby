use std::{fs::File, path::Path};

use itertools::Itertools;
use magnum::container::ogg::OpusSourceOgg;
use whisper_rs::{
  FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperError,
};

pub struct AudioHandler<'a> {
  ctx: WhisperContext,
  params: FullParams<'a, 'a>,
}

impl<'a> AudioHandler<'a> {
  pub fn new<S: AsRef<str>>(model_path: S) -> anyhow::Result<AudioHandler<'a>> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    params.set_detect_language(true);
    params.set_print_progress(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);

    let ctx =
      WhisperContext::new_with_params(model_path.as_ref(), WhisperContextParameters::default())?;

    Ok(AudioHandler { ctx, params })
  }

  pub fn tts(&self, input: &[u8]) -> anyhow::Result<String> {
    let cursor = std::io::Cursor::new(input);
    let source = OpusSourceOgg::new(cursor)?;
    // source is f32 at 48kHz; we need to conver it to f32 at 16kHz.
    // 48/16 is 3 so convert each 3 samples into 1.
    let converted = source
      .chunks(3)
      .into_iter()
      .map(|chunk| chunk.sum::<f32>() / 3.0)
      .collect::<Vec<_>>();

    let mut state = self.ctx.create_state()?;

    state.full(self.params.clone(), &converted[..])?;

    let segment_count = state.full_n_segments()?;
    let text = (0..segment_count)
      .map(|i| state.full_get_segment_text(i))
      .collect::<Result<Vec<String>, WhisperError>>()?
      .join(" ");

    Ok(text)
  }

  pub fn ensure_model(model_file: &Path) {
    if model_file.exists() {
      return;
    }

    let url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";
    let mut rdr = ureq::get(url).call().unwrap().into_reader();

    std::io::copy(&mut rdr, &mut File::create(model_file).unwrap()).unwrap();
  }
}
