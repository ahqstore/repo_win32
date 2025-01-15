mod parser;

use parser::*;

#[tokio::main]
async fn main() {
  parser().await;
}
