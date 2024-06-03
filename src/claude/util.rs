use image::{imageops::FilterType, io::Reader as ImageReader, GenericImageView};
use std::io::Cursor;

pub fn resize_image(bytes: Vec<u8>, max_w: u32, max_h: u32) -> anyhow::Result<Vec<u8>> {
  let cursor = Cursor::new(&bytes);
  let img = ImageReader::new(cursor).with_guessed_format()?.decode()?;

  let (w, h) = img.dimensions();

  if w < max_w && h < max_h {
    return Ok(bytes);
  }

  let img = img.resize(max_w, max_h, FilterType::Triangle);

  Ok(img.into_bytes())
}
