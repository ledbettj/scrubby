use sonata_piper::PiperSynthesisConfig;
use sonata_synth::{SonataModel, SonataSpeechSynthesizer};
use whisper_rs::{FullParams, SamplingStrategy, WhisperState};

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

pub fn recognize(wisp_state: &mut WhisperState, raw_data: &[f32]) -> String {
  let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });
  params.set_n_threads(16);
  // Enable translation.
  params.set_translate(true);
  // Set the language to translate to to English.
  params.set_language(Some("en"));
  // Disable anything that prints to stdout.
  params.set_print_special(false);
  params.set_print_progress(false);
  params.set_print_realtime(false);
  params.set_print_timestamps(false);

  // std::fs::write("/home/john/out32.wav", &raw_to_wav32(&raw_data));
  wisp_state
    .full(params, &raw_data[..])
    .expect("Failed to run model");

  let segment_count = wisp_state.full_n_segments().unwrap();

  let transcript = (0..segment_count)
    .map(|index| wisp_state.full_get_segment_text(index).unwrap())
    .collect::<String>();

  transcript
}

pub fn init_synth() -> (SonataSpeechSynthesizer, usize) {
  let path = std::path::Path::new("./models/piper/ryan/high/en_US-ryan-high.onnx.json");
  let json: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
  let sample_rate_value = json.get("audio").and_then(|audio| audio.get("sample_rate"));
  let sample_rate = if let Some(serde_json::Value::Number(n)) = sample_rate_value {
    n.as_u64().unwrap()
  } else {
    panic!("cant read sample rate");
  };
  let voice = sonata_piper::from_config_path(path).expect("Cant get voice");
  let synth = SonataSpeechSynthesizer::new(voice).expect("Cant create synth");
  let mut cfg: PiperSynthesisConfig = *synth
    .get_default_synthesis_config()
    .unwrap()
    .downcast()
    .expect("Expected piper config");
  cfg.speaker = None;

  synth.set_fallback_synthesis_config(&cfg).unwrap();
  (synth, sample_rate as usize)
}

pub fn generate(synth: &SonataSpeechSynthesizer, sample_rate: usize, input: &str) -> Vec<u8> {
  let stream = synth.synthesize_lazy(input.to_string(), None).unwrap();

  let mut data: Vec<u8> = Vec::with_capacity(sample_rate * 30);
  let mut wav = wav_header(sample_rate, 1, 2);

  for result in stream {
    let audio = result.unwrap();
    let wav_bytes = audio.as_wave_bytes();
    data.extend(&wav_bytes);
  }
  wav_set_lengths(&mut wav, data.len());
  wav.extend(&data);
  wav
}

fn wav_set_lengths(header: &mut [u8], data_size: usize) {
  let fsize = 44 + data_size;
  header[4] = (fsize & 0xFF) as u8;
  header[5] = (fsize >> 8) as u8;
  header[6] = (fsize >> 16) as u8;
  header[7] = (fsize >> 24) as u8;

  header[40] = (data_size & 0xFF) as u8;
  header[41] = (data_size >> 8) as u8;
  header[42] = (data_size >> 16) as u8;
  header[43] = (data_size >> 24) as u8;
}

fn wav_header(sample_rate: usize, channels: u8, sample_width: usize) -> Vec<u8> {
  let bits_per_sample = 8 * sample_width;
  let mul = sample_rate * bits_per_sample * (channels as usize) / 8;
  let mul2 = bits_per_sample * (channels as usize) / 8;
  vec![
    'R' as u8,
    'I' as u8,
    'F' as u8,
    'F' as u8,
    0,
    0,
    0,
    0, // file size place holder
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
    0, // header size
    1,
    0, // pcm
    channels,
    0,
    (sample_rate & 0xFF) as u8,
    (sample_rate >> 8) as u8,
    (sample_rate >> 16) as u8,
    (sample_rate >> 24) as u8,
    (mul & 0xFF) as u8,
    (mul >> 8) as u8,
    (mul >> 16) as u8,
    (mul >> 24) as u8,
    (mul2 & 0xFF) as u8,
    (mul2 >> 8) as u8,
    (bits_per_sample & 0xFF) as u8,
    (bits_per_sample >> 8) as u8,
    'd' as u8,
    'a' as u8,
    't' as u8,
    'a' as u8,
    0,
    0,
    0,
    0, // data size placeholder
  ]
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
