use image::{
  codecs::png::{self, PngEncoder},
  imageops::FilterType,
  GenericImageView, ImageReader,
};
use log::debug;
use std::io::Cursor;

pub fn resize_image(bytes: Vec<u8>, max_w: u32, max_h: u32) -> anyhow::Result<Vec<u8>> {
  let cursor = Cursor::new(&bytes);
  let mut img = ImageReader::new(cursor).with_guessed_format()?.decode()?;

  let (w, h) = img.dimensions();

  if w > max_w || h > max_h {
    debug!("Resizing image: {:?} > {:?}", (w, h), (max_w, max_h));
    img = img.resize(max_w, max_h, FilterType::Triangle);
  }
  let mut output = vec![];
  let mut writer = Cursor::new(&mut output);
  let encoder = PngEncoder::new_with_quality(
    &mut writer,
    png::CompressionType::Best,
    png::FilterType::Adaptive,
  );
  img.write_with_encoder(encoder)?;

  Ok(output)
}
