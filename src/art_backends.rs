use crossbeam::channel::Sender;
use image::DynamicImage;
use mljcl::{types::Album, credentials::MalojaCredentials};
use rascii_art::RenderOptions;

use crate::{ALBUM_WIDTH, MAX_ALBUM_HEIGHT};

use bytes::Bytes;

#[derive(Clone)]
pub struct CachedRender {
    pub art: String,
    pub size: (u32, u32),
}

#[derive(Clone)]
pub struct AlbumArt {
    pub album_id: String,
    pub name: String,
    pub art: Option<CachedRender>,
    pub image: DynamicImage,
}

pub fn truncate(string: String, max_len: usize) -> String {
    if string.len() <= max_len {
        string
    } else {
        string.split_at(max_len - 1).0.to_owned() + "…"
    }
}

impl AlbumArt {
    pub fn display_string(&self) -> String {
        match &self.art {
            Some(render) => {
                render.art.clone()
                    + "\n"
                    + &truncate(self.clone().name, render.size.1.try_into().unwrap())
            }
            None => "".to_string(),
        }
    }
    #[allow(dead_code)]
    pub fn display_string_with_size(&mut self, height: u32, width: u32) -> String {
        match self.art.clone() {
            Some(render) => {
                if height == 1 {
                    return truncate(self.clone().name, width.try_into().unwrap());
                }
                if render.size != (height, width) {
                    self.art = Some(CachedRender {
                        art: rascii(self.image.clone(), height, width),
                        size: (height, width),
                    });
                }
                self.display_string()
            }
            None => "".to_string(),
        }
    }
}

pub fn get_image(data: Bytes) -> Option<DynamicImage> {
    if let Ok(img) = image::load_from_memory(&data) {
        Some(img)
    } else {
        None
    }
}

pub fn rascii(image: DynamicImage, height: u32, width: u32) -> String {
    let mut ret: String = "".to_string();
    let render_options = RenderOptions::new()
        .charset(rascii_art::charsets::BLOCK)
        .colored(true)
        .height(height)
        .width(width);
    let _ = rascii_art::render_image_to(&image, &mut ret, &render_options);
    ret
}

pub async fn get_art_for(
    album: (Album, u64),
    sender: Sender<AlbumArt>,
    credentials: MalojaCredentials,
    client: reqwest::Client,
) {
    let id = album.0.id;
    let bytes = mljcl::art::album_art_async(id.clone(), credentials, client)
        .await
        .unwrap();
    match get_image(bytes) {
        Some(image) => {
            let bytes = rascii(image.clone(), MAX_ALBUM_HEIGHT, ALBUM_WIDTH);
            sender
                .send(AlbumArt {
                    album_id: id,
                    art: Some(CachedRender {
                        art: bytes,
                        size: (MAX_ALBUM_HEIGHT, ALBUM_WIDTH),
                    }),
                    name: album.0.name,
                    image,
                })
                .expect("Error sending Album Art between threads");
            drop(sender);
        }
        None => {
            drop(sender);
        }
    }
}
