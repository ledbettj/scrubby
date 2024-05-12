use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub fn convert_pcm(
  input: &[i16],
  input_hz: usize,
  input_channels: usize,
  output_hz: usize,
) -> Vec<f32> {
  let window_size = input_hz / output_hz * input_channels;

  input
    .chunks_exact(window_size)
    .map(|chunk| {
      chunk.iter().map(|v| *v as f32).sum::<f32>() / ((window_size * i16::MAX as usize) as f32)
    })
    .collect()
}

pub fn recognize(raw_data: &[f32]) -> String {
  let ctx = WhisperContext::new_with_params(
    "./models/whisper.cpp-model-medium.en/ggml-medium.en.bin",
    WhisperContextParameters::default(),
  )
  .expect("Failed to load model");

  let mut state = ctx.create_state().expect("Failed to create key");
  let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });
  params.set_n_threads(8);
  // Enable translation.
  params.set_translate(true);
  // Set the language to translate to to English.
  params.set_language(Some("en"));
  // Disable anything that prints to stdout.
  params.set_print_special(false);
  params.set_print_progress(false);
  params.set_print_realtime(false);
  params.set_print_timestamps(false);

  std::fs::write("/home/john/out32.wav", &raw_to_wav32(&raw_data));
  state
    .full(params, &raw_data[..])
    .expect("Failed to run model");

  let segment_count = state.full_n_segments().unwrap();

  let transcript = (0..segment_count)
    .map(|index| state.full_get_segment_text(index).unwrap())
    .collect::<String>();

  transcript
}

pub fn raw_to_wav(voice_data: &[i16]) -> Vec<u8> {
  let fsize = 44 + voice_data.len() * 2;
  let dsize = voice_data.len() * 2;
  let rate = 16000;
  let mul = rate * 16 * 2 / 8;
  let mut data = vec![
    'R' as u8,
    'I' as u8,
    'F' as u8,
    'F' as u8,
    (fsize & 0xFF) as u8,
    (fsize >> 8) as u8,
    (fsize >> 16) as u8,
    (fsize >> 24) as u8,
    'W' as u8,
    'A' as u8,
    'V' as u8,
    'E' as u8,
    'f' as u8,
    'm' as u8,
    't' as u8,
    ' ' as u8,
    16,
    0,
    0,
    0,
    1,
    0,
    2,
    0,
    (rate & 0xFF) as u8,
    (rate >> 8) as u8,
    (rate >> 16) as u8,
    (rate >> 24) as u8,
    (mul & 0xFF) as u8,
    (mul >> 8) as u8,
    (mul >> 16) as u8,
    (mul >> 24) as u8,
    (16 * 2) / 8 as u8,
    0,
    16,
    0,
    'd' as u8,
    'a' as u8,
    't' as u8,
    'a' as u8,
    (dsize & 0xFF) as u8,
    (dsize >> 8) as u8,
    (dsize >> 16) as u8,
    (dsize >> 24) as u8,
  ];

  unsafe {
    let slice = std::slice::from_raw_parts(voice_data.as_ptr() as *const u8, voice_data.len() * 2);
    data.extend(slice)
  }

  data
}

pub fn raw_to_wav32(voice_data: &[f32]) -> Vec<u8> {
  let fsize = 44 + voice_data.len() * 4;
  let dsize = voice_data.len() * 4;
  let rate = 16000;
  let mul = rate * 32 / 8;
  let mut data = vec![
    'R' as u8,
    'I' as u8,
    'F' as u8,
    'F' as u8,
    (fsize & 0xFF) as u8,
    (fsize >> 8) as u8,
    (fsize >> 16) as u8,
    (fsize >> 24) as u8,
    'W' as u8,
    'A' as u8,
    'V' as u8,
    'E' as u8,
    'f' as u8,
    'm' as u8,
    't' as u8,
    ' ' as u8,
    16,
    0,
    0,
    0,
    1,
    0,
    1,
    0,
    (rate & 0xFF) as u8,
    (rate >> 8) as u8,
    (rate >> 16) as u8,
    (rate >> 24) as u8,
    (mul & 0xFF) as u8,
    (mul >> 8) as u8,
    (mul >> 16) as u8,
    (mul >> 24) as u8,
    (32 * 1) / 8 as u8,
    0,
    32,
    0,
    'd' as u8,
    'a' as u8,
    't' as u8,
    'a' as u8,
    (dsize & 0xFF) as u8,
    (dsize >> 8) as u8,
    (dsize >> 16) as u8,
    (dsize >> 24) as u8,
  ];

  unsafe {
    let slice = std::slice::from_raw_parts(voice_data.as_ptr() as *const u8, voice_data.len() * 4);
    data.extend(slice)
  }

  data
}
