use serde::{Deserialize, Deserializer, Serialize};
use serde::de::Error as DError;
use serde_json::Value;

#[derive(Deserialize, Serialize, Debug)]
pub struct Gallery {
    #[serde(deserialize_with="num_or_str_num")]
    pub id: u32,
    pub media_id: String,
    pub title: GalleryTitle,
    pub images: GalleryImages,
    pub tags: Vec<GalleryTag>,
    pub num_favorites: u32,
    pub upload_date: u64,
}

impl Gallery {
    pub fn pages(&self) -> usize {
        self.images.pages.len()
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GalleryTitle {
    #[serde(deserialize_with="default_on_null")]
    pub english: String,
    #[serde(deserialize_with="default_on_null")]
    pub japanese: String,
    #[serde(deserialize_with="default_on_null")]
    pub pretty: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GalleryImages {
    #[serde(deserialize_with="tag_to_untagged_vec")]
    pub pages: Vec<ImageType>,
    #[serde(deserialize_with="tag_to_untagged")]
    pub cover: ImageType,
    #[serde(deserialize_with="tag_to_untagged")]
    pub thumbnail: ImageType,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub enum ImageType {
    #[serde(rename = "w")]
    Webp,
    #[serde(rename = "j")]
    Jpg,
    #[serde(rename = "p")]
    Png,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GalleryTag {
    pub id: u32,
    pub name: String,
}

fn num_or_str_num<'de, D>(d: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
    D::Error: DError,
{
    let value = Value::deserialize(d)?;
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        .ok_or(DError::custom("Not a number nor a number string"))?
        .try_into()
        .map_err(|_| DError::custom("Number overflow"))
}

fn default_on_null<'de, D, T>(d: D) -> std::result::Result<T, D::Error>
where 
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let r = Option::deserialize(d)?;
    Ok(r.unwrap_or_default())
}

fn tag_to_untagged_single<E>(value: &Value) -> std::result::Result<ImageType, E>
where
    E: DError,
{
    let t = value.as_object()
        .and_then(|o| o.get("t"))
        .and_then(|v| v.as_str())
        .or_else(|| value.as_str())
        .ok_or(DError::custom("Value is not a tag struct nor a string"))?;

    match t {
        "w" => Ok(ImageType::Webp),
        "j" => Ok(ImageType::Jpg),
        "p" => Ok(ImageType::Png),
        _ => Err(DError::custom("Invalid ImageType"))
    }
}

fn tag_to_untagged<'de, D>(d: D) -> std::result::Result<ImageType, D::Error>
where 
    D: Deserializer<'de>,
    D::Error: DError,
{
    let value = Value::deserialize(d)?;
    tag_to_untagged_single(&value)
}

fn tag_to_untagged_vec<'de, D>(d: D) -> std::result::Result<Vec<ImageType>, D::Error>
where 
    D: Deserializer<'de>,
    D::Error: DError,
{
    let value = Value::deserialize(d)?;
    let t = value.as_array()
        .ok_or(DError::custom("Expected array"))?;

    t.iter()
        .map(tag_to_untagged_single)
        .collect()
}
