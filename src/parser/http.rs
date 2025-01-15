use reqwest::{Client, ClientBuilder};
use content_disposition::parse_content_disposition;
use std::sync::LazyLock;

static CLIENT: LazyLock<Client> = LazyLock::new(|| ClientBuilder::new().build().unwrap());

pub async fn cnt_dsp_check(url: &str, ends_with: &str) -> bool {
  cnt_dsp_inner(url, ends_with).await.unwrap_or(false)
}

async fn cnt_dsp_inner(url: &str, ends: &str) -> Option<bool> {
  let res = CLIENT.get(url).send().await.ok()?;

  let headers = res.headers();

  let cnt = headers.get("Content-Disposition").or_else(|| headers.get("content-disposition"))?;
  let cnt = cnt.to_str().ok()?;

  let cnt = parse_content_disposition(cnt);

  drop(res);

  Some(cnt.filename_full()?.ends_with(ends))
}