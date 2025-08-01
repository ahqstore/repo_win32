use content_disposition::parse_content_disposition;
use reqwest::{Client, ClientBuilder};
use std::sync::LazyLock;
use std::time::Duration;

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
  ClientBuilder::new()
    .connect_timeout(Duration::from_secs(30))
    .build()
    .unwrap()
});

pub async fn cnt_dsp_check(url: &str, ends_with: &str) -> bool {
  cnt_dsp_inner(url, ends_with).await.unwrap_or(false)
}

async fn cnt_dsp_inner(url: &str, ends: &str) -> Option<bool> {
  let res = CLIENT.head(url).send().await.ok()?;

  let headers = res.headers();

  let cnt = headers
    .get("Content-Disposition")
    .or_else(|| headers.get("content-disposition"))?;
  let cnt = cnt.to_str().ok()?;

  let cnt = parse_content_disposition(cnt);

  let res = cnt.filename_full()?.to_lowercase().ends_with(ends);

  drop(cnt);

  Some(res)
}
